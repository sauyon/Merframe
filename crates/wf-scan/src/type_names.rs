//! Names of the game's types, read the way its own Lua bindings read them. `Object:GetType`
//! loads an object's type from a field of the object; `ObjectType:GetName` loads a name id
//! from a field of the type and hands it to a lookup that splits it into a string pool's index
//! (the low 16 bits, into a table of 16-byte entries) and an offset into that pool (the high
//! 16 bits). Where the fields and the table are is read out of that code.

use wf_mem::{MemoryReader, read_u32_le};

use crate::swig::Image;

const OBJECT: &str = "Object";
const GET_TYPE: &str = "GetType";
const OBJECT_TYPE: &str = "ObjectType";
const GET_NAME: &str = "GetName";
/// How much of a binding, or of the name lookup, to read for the instructions below.
const SPAN: usize = 0x80;
const CALL: u8 = 0xE8;
/// `mov rcx, [rip+disp32]`: the lookup loading its table.
const LOAD_TABLE: [u8; 3] = [0x48, 0x8B, 0x0D];
/// `shl rdx, 4`; `shr rax, 0x10`: the lookup indexing 16-byte entries by the low half.
const INDEX: [u8; 4] = [0x48, 0xC1, 0xE2, 0x04];
const OFFSET: [u8; 4] = [0x48, 0xC1, 0xE8, 0x10];
const MAX_NAME: usize = 128;
const PAGE: u64 = 0x1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeNames {
    /// The object's field holding its type.
    type_field: u64,
    /// The type's field holding its name id.
    name_field: u64,
    /// The word holding the string pools' table.
    table: u64,
}

impl TypeNames {
    /// `None` when the game's code no longer matches what this expects.
    pub fn locate(image: &Image) -> Option<Self> {
        let get_type = image.class(OBJECT)?.method(image, GET_TYPE)?;
        let type_field = field(image.code(get_type, SPAN)?, true)?.1;
        let get_name = image.class(OBJECT_TYPE)?.method(image, GET_NAME)?;
        let code = image.code(get_name, SPAN)?;
        let (end, name_field) = field(code, false)?;
        let call = end + code[end..].iter().position(|&byte| byte == CALL)?;
        let rel = i64::from(read_u32_le(code, call + 1)?.cast_signed());
        let lookup = get_name.checked_add_signed(i64::try_from(call + 5).ok()? + rel)?;
        let code = image.code(lookup, SPAN)?;
        let load = memchr::memmem::find(code, &LOAD_TABLE)?;
        if memchr::memmem::find(code, &INDEX).is_none()
            || memchr::memmem::find(code, &OFFSET).is_none()
        {
            return None;
        }
        let rel = i64::from(read_u32_le(code, load + 3)?.cast_signed());
        let table = lookup.checked_add_signed(i64::try_from(load + 7).ok()? + rel)?;
        Some(Self {
            type_field,
            name_field,
            table,
        })
    }

    #[cfg(test)]
    pub(crate) fn at(type_field: u64, name_field: u64, table: u64) -> Self {
        Self {
            type_field,
            name_field,
            table,
        }
    }

    /// The name of the object's type, `GeodeShardsAbility`.
    pub fn of_object<R: MemoryReader + ?Sized>(&self, reader: &R, object: u64) -> Option<String> {
        let kind = reader.read_u64(object + self.type_field).ok()?;
        self.of_type(reader, kind)
    }

    pub fn of_type<R: MemoryReader + ?Sized>(&self, reader: &R, kind: u64) -> Option<String> {
        let id = read_u32_le(&reader.read_vec(kind + self.name_field, 4).ok()?, 0)?;
        let pools = reader.read_u64(self.table).ok()?;
        let pool = reader.read_u64(pools + u64::from(id & 0xffff) * 16).ok()?;
        text(reader, pool + u64::from(id >> 16))
    }
}

/// The first `mov reg, [rax+disp8]` in `code`, 64-bit or 32-bit: where it ends, and its disp.
fn field(code: &[u8], wide: bool) -> Option<(usize, u64)> {
    let len = if wide { 4 } else { 3 };
    (0..=code.len().saturating_sub(len)).find_map(|at| {
        let op = if wide {
            let [0x48, 0x8B, modrm, disp, ..] = code[at..] else {
                return None;
            };
            (modrm, disp)
        } else {
            let [0x8B, modrm, disp, ..] = code[at..] else {
                return None;
            };
            if at > 0 && code[at - 1] & 0xF0 == 0x40 {
                return None;
            }
            (modrm, disp)
        };
        let (modrm, disp) = op;
        // mod 01 (disp8), r/m 000 (rax)
        (modrm & 0xC7 == 0x40).then_some((at + len, u64::from(disp)))
    })
}

/// A NUL-terminated identifier, read without crossing into a page that may not be mapped.
fn text<R: MemoryReader + ?Sized>(reader: &R, at: u64) -> Option<String> {
    let to_page = usize::try_from(PAGE - (at % PAGE)).ok()?;
    let bytes = reader
        .read_vec(at, MAX_NAME)
        .or_else(|_| reader.read_vec(at, to_page.min(MAX_NAME)))
        .ok()?;
    let end = bytes.iter().position(|&byte| byte == 0)?;
    let name = std::str::from_utf8(&bytes[..end]).ok()?;
    (!name.is_empty() && name.bytes().all(|byte| byte.is_ascii_graphic())).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_type_and_name_fields() {
        // call ...; mov rcx, rbx; mov rdx, [rax+8]
        let get_type = [0xE8, 0, 0, 0, 0, 0x48, 0x8B, 0xCB, 0x48, 0x8B, 0x50, 0x08];
        assert_eq!(field(&get_type, true), Some((12, 8)));
        // lea rcx, [rsp+0x38]; mov edx, [rax+0x2c]
        let get_name = [0x48, 0x8D, 0x4C, 0x24, 0x38, 0x8B, 0x50, 0x2C, 0x89];
        assert_eq!(field(&get_name, false), Some((8, 0x2c)));
    }
}
