//! The game's SWIG Lua bindings, read back by name: which native classes Lua sees, the
//! functions behind their methods, and the live objects Lua holds of each class.
//!
//! Each bound class has a `swig_type_info` in the image's data whose first word points at its
//! mangled name, `_p_<Class>`; its fourth word points at the class record, whose words at
//! [`CLASS_METHODS`] and after point at tables of `{hash, function}` pairs ending in a null
//! function. Method names survive only as [`member_hash`] values. A Lua userdata holding an
//! object carries the type info, tagged, and a handle box whose first word is the object.

use memchr::memmem;
use wf_mem::{GAME_PROCESS, MemoryReader, read_u32_le, read_u64_le};

use crate::Result;
use crate::chunks::read_region;
use crate::roots::{POINTER, heap_regions, words};

const TYPE_CLASS: u64 = 0x18;
const CLASS_METHODS: u64 = 0x20;
const METHOD: u64 = 16;
const MAX_METHODS: usize = 1024;
const USERDATA_TAG: u8 = 9;
const USERDATA_TYPE: usize = 0x10;
const USERDATA_BOX: usize = 0x18;
/// Low bits the game sets on the type info pointer it keeps in a userdata.
const TYPE_TAG: u64 = 3;
const CALL: u8 = 0xE8;
const RET_PAD: [u8; 2] = [0xC3, 0xCC];
/// How far into a binding to look for the call into the method's implementation.
const WRAPPER_SPAN: usize = 0x400;

/// The hash the bindings key member names by: FNV-1a from a game-specific basis, inverted
/// and rotated.
pub fn member_hash(name: &str) -> u32 {
    let mixed = name.bytes().fold(0x768e_5ed0_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    (!mixed).rotate_left(17)
}

/// The game's executable image as mapped, with unreadable pages left zero.
pub struct Image {
    pub base: u64,
    bytes: Vec<u8>,
}

impl Image {
    pub fn read<R: MemoryReader + ?Sized>(reader: &R) -> Result<Self> {
        let module = reader.module(GAME_PROCESS)?;
        let mut bytes = vec![0u8; crate::chunks::to_usize(module.size)?];
        for region in reader.regions()? {
            let start = region.range.start.max(module.base);
            let end = region.range.end.min(module.base + module.size);
            if !region.readable || start >= end {
                continue;
            }
            let at = crate::chunks::to_usize(start - module.base)?;
            let len = crate::chunks::to_usize(end - start)?;
            let _ = reader.read(start, &mut bytes[at..at + len]);
        }
        Ok(Self {
            base: module.base,
            bytes,
        })
    }

    #[cfg(test)]
    pub fn from_bytes(base: u64, bytes: Vec<u8>) -> Self {
        Self { base, bytes }
    }

    fn offset(&self, address: u64) -> Option<usize> {
        let at = usize::try_from(address.checked_sub(self.base)?).ok()?;
        (at < self.bytes.len()).then_some(at)
    }

    fn word(&self, address: u64) -> Option<u64> {
        read_u64_le(&self.bytes, self.offset(address)?)
    }

    fn holds(&self, address: u64) -> bool {
        self.offset(address).is_some()
    }

    pub fn code(&self, address: u64, len: usize) -> Option<&[u8]> {
        let at = self.offset(address)?;
        self.bytes.get(at..(at + len).min(self.bytes.len()))
    }

    /// The type info of the bound class `class`, e.g. `LotusHudRedux`.
    pub fn class(&self, class: &str) -> Option<SwigClass> {
        let mangled = format!("\0_p_{class}\0");
        let name = memmem::find_iter(&self.bytes, mangled.as_bytes())
            .map(|at| self.base + at as u64 + 1)
            .collect::<Vec<_>>();
        words(&self.bytes)
            .filter(|(_, word)| name.contains(word))
            .map(|(at, _)| self.base + at as u64)
            .find_map(|type_info| {
                let record = self.word(type_info + TYPE_CLASS)?;
                let methods = self.word(record + CLASS_METHODS)?;
                self.holds(methods)
                    .then_some(SwigClass { type_info, methods })
            })
    }

    /// The function a binding hands its work to: its last call before it returns.
    pub fn callee(&self, wrapper: u64) -> Option<u64> {
        let code = self.code(wrapper, WRAPPER_SPAN)?;
        let end = memmem::find(code, &RET_PAD)?;
        (0..end.saturating_sub(4))
            .rev()
            .filter(|&at| code[at] == CALL)
            .find_map(|at| {
                let rel = i64::from(i32::from_le_bytes(
                    code.get(at + 1..at + 5)?.try_into().ok()?,
                ));
                let target = wrapper.checked_add_signed(i64::try_from(at + 5).ok()? + rel)?;
                self.holds(target).then_some(target)
            })
    }
}

/// A class the Lua bindings know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwigClass {
    pub type_info: u64,
    methods: u64,
}

impl SwigClass {
    /// The class's methods, as `(name hash, binding)`.
    pub fn methods(&self, image: &Image) -> Vec<(u32, u64)> {
        (0..MAX_METHODS as u64)
            .map(|index| self.methods + index * METHOD)
            .map_while(|entry| {
                let function = image.word(entry + 8)?;
                let key = u32::try_from(image.word(entry)? & 0xffff_ffff).ok()?;
                (function != 0).then_some((key, function))
            })
            .collect()
    }

    /// The binding behind the class's method `name`.
    pub fn method(&self, image: &Image, name: &str) -> Option<u64> {
        let hash = member_hash(name);
        self.methods(image)
            .into_iter()
            .find(|&(key, _)| key == hash)
            .map(|(_, function)| function)
    }

    /// Every native object Lua holds of this class, as `(handle box, object)`.
    pub fn instances<R: MemoryReader + ?Sized>(&self, reader: &R) -> Result<Vec<(u64, u64)>> {
        let tagged = self.type_info | TYPE_TAG;
        let regions = reader.regions()?;
        let mut found = Vec::new();
        for region in heap_regions(&regions) {
            let Ok(data) = read_region(reader, region) else {
                continue;
            };
            for (at, word) in words(&data) {
                let Some(start) = at.checked_sub(USERDATA_TYPE) else {
                    continue;
                };
                if word != tagged || data[start] != USERDATA_TAG {
                    continue;
                }
                let Some(handle) = read_u64_le(&data, start + USERDATA_BOX) else {
                    continue;
                };
                if let Ok(object) = reader.read_u64(handle)
                    && object != 0
                    && object.is_multiple_of(POINTER as u64)
                    && !found.contains(&(handle, object))
                {
                    found.push((handle, object));
                }
            }
        }
        Ok(found)
    }
}

/// Fields of `this` an implementation reads as `{pointer, u32 bytes, u32 capacity}` arrays:
/// a byte count loaded and shifted down by `log2(stride)`, the way the game counts elements.
pub fn counted_arrays(code: &[u8], stride_log2: u8) -> Vec<u32> {
    let mut found = Vec::new();
    for at in 0..code.len() {
        let (rex, rest) = match code[at] {
            rex @ 0x40..=0x47 => (rex, &code[at + 1..]),
            _ => (0x40, &code[at..]),
        };
        let [0x8B, modrm, ..] = rest else { continue };
        if modrm >> 6 != 2 || modrm & 7 == 4 {
            continue;
        }
        let Some(disp) = read_u32_le(rest, 2) else {
            continue;
        };
        let dest = ((rex >> 2) & 1) << 3 | (modrm >> 3) & 7;
        let [shift_rex, 0xC1, shift_modrm, amount, ..] = rest[6..] else {
            continue;
        };
        let shifted = (shift_rex & 1) << 3 | shift_modrm & 7;
        if shift_rex & 0xF8 == 0x48
            && shift_modrm >> 3 == 0x1D
            && amount == stride_log2
            && shifted == dest
            && !found.contains(&disp)
        {
            found.push(disp.wrapping_sub(8));
        }
    }
    found
}

/// Fields of `this` an implementation indexes as arrays of 16-byte elements: an index
/// shifted up by 4 and added to the array pointer loaded from the field.
pub fn indexed_arrays(code: &[u8]) -> Vec<u32> {
    let mut found = Vec::new();
    for at in 0..code.len().saturating_sub(11) {
        let [shift_rex, 0xC1, shift_modrm, 4, add_rex, 0x03, modrm, ..] = code[at..] else {
            continue;
        };
        if shift_rex & 0xF8 != 0x48 || shift_modrm >> 3 != 0x1C || add_rex & 0xF8 != 0x48 {
            continue;
        }
        let index = (shift_rex & 1) << 3 | shift_modrm & 7;
        let dest = ((add_rex >> 2) & 1) << 3 | (modrm >> 3) & 7;
        if modrm >> 6 != 2 || modrm & 7 == 4 || index != dest {
            continue;
        }
        if let Some(disp) = read_u32_le(code, at + 7)
            && !found.contains(&disp)
        {
            found.push(disp);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_the_game() {
        assert_eq!(member_hash("GetSuit"), 0x7234_d87e);
        assert_eq!(member_hash("IsActive"), 0xe95a_eb3c);
        assert_eq!(member_hash("AddAbilityTimer"), 0x1eb6_d153);
    }

    #[test]
    fn finds_counts_and_indexed_arrays() {
        // mov r10d, [r14+0xd8]; shr r10, 3; shl rdi, 4; add rdi, [r14+0x478]
        let code = [
            0x45, 0x8B, 0x96, 0xD8, 0, 0, 0, 0x49, 0xC1, 0xEA, 0x03, 0x90, 0x48, 0xC1, 0xE7, 0x04,
            0x49, 0x03, 0xBE, 0x78, 0x04, 0, 0, 0x90,
        ];
        assert_eq!(counted_arrays(&code, 3), [0xd0]);
        assert_eq!(indexed_arrays(&code), [0x478]);
        // mov eax, [rcx+0x3460]; shr rax, 3
        let suit = [0x8B, 0x81, 0x60, 0x34, 0, 0, 0x48, 0xC1, 0xE8, 0x03];
        assert_eq!(counted_arrays(&suit, 3), [0x3458]);
    }

    #[test]
    fn reads_classes_methods_and_callees() {
        let base: u64 = 0x1_4000_0000;
        let mut bytes = vec![0u8; 0x1000];
        bytes[0x101..0x111].copy_from_slice(b"_p_LotusHudRedux");
        // type info at 0x200 -> name 0x101, class record at 0x300 -> methods at 0x400
        bytes[0x200..0x208].copy_from_slice(&(base + 0x101).to_le_bytes());
        bytes[0x218..0x220].copy_from_slice(&(base + 0x300).to_le_bytes());
        bytes[0x320..0x328].copy_from_slice(&(base + 0x400).to_le_bytes());
        bytes[0x400..0x404].copy_from_slice(&member_hash("Update").to_le_bytes());
        bytes[0x408..0x410].copy_from_slice(&(base + 0x900).to_le_bytes());
        bytes[0x410..0x414].copy_from_slice(&member_hash("AddAbilityTimer").to_le_bytes());
        bytes[0x418..0x420].copy_from_slice(&(base + 0x800).to_le_bytes());
        // the binding at 0x800 calls 0x880 last, then returns
        bytes[0x800] = CALL;
        bytes[0x801..0x805].copy_from_slice(&0x100_i32.to_le_bytes());
        bytes[0x805] = CALL;
        bytes[0x806..0x80a].copy_from_slice(&0x76_i32.to_le_bytes());
        bytes[0x80a..0x80c].copy_from_slice(&RET_PAD);
        let image = Image::from_bytes(base, bytes);

        let class = image.class("LotusHudRedux").expect("class");
        assert_eq!(class.type_info, base + 0x200);
        let wrapper = class.method(&image, "AddAbilityTimer").expect("method");
        assert_eq!(wrapper, base + 0x800);
        assert_eq!(class.method(&image, "Missing"), None);
        assert_eq!(image.callee(wrapper), Some(base + 0x880));
    }
}
