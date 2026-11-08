//! Cooldowns the game keeps on the suit's abilities themselves, like the wait after Koumei's
//! Omamori saves her: an ability keeps the seconds its cooldown has left, which it counts down
//! and is on cooldown while above zero, and the cooldown's unscaled length.
//!
//! Ability scripts read and set both through `PowerSuitAbility`'s Lua bindings.
//! `GetCooldownTime` loads the seconds left straight off the ability and hands them to Lua;
//! `SetCooldownDuration` stores what Lua gives it straight into the length. Where both are is
//! read out of those bindings.

use wf_mem::{MemoryReader, read_u32_le};

use crate::swig::Image;

const ABILITY_CLASS: &str = "PowerSuitAbility";
const GET_TIME: &str = "GetCooldownTime";
const SET_DURATION: &str = "SetCooldownDuration";
/// How much of a binding to read for the field it loads or stores.
const BINDING_SPAN: usize = 0x80;
/// `movss xmm, [reg+disp32]` and `movss [reg+disp32], xmm`.
const MOVSS_LOAD: u8 = 0x10;
const MOVSS_STORE: u8 = 0x11;
/// Longer than any ability's cooldown.
const MAX_SECONDS: f32 = 3600.0;

/// A cooldown running on one of the suit's abilities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityCooldown {
    /// Which of the suit's abilities, from 0, in the suit's own order.
    pub ability: usize,
    /// Seconds left.
    pub remaining: f32,
    /// Its unscaled length in seconds.
    pub duration: f32,
}

/// Where an ability keeps its cooldown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CooldownFields {
    time: u64,
    duration: u64,
}

impl CooldownFields {
    /// `None` when the game's code no longer matches what this expects.
    pub fn locate(image: &Image) -> Option<Self> {
        let class = image.class(ABILITY_CLASS)?;
        let binding = |name| image.code(class.method(image, name)?, BINDING_SPAN);
        let time = movss(binding(GET_TIME)?, MOVSS_LOAD)?;
        let duration = movss(binding(SET_DURATION)?, MOVSS_STORE)?;
        (time != duration).then_some(Self {
            time: u64::from(time),
            duration: u64::from(duration),
        })
    }

    #[cfg(test)]
    pub(crate) fn at(time: u64, duration: u64) -> Self {
        Self { time, duration }
    }

    /// The cooldown running on the ability object `object`, the suit's `ability`th.
    pub fn read<R: MemoryReader + ?Sized>(
        &self,
        reader: &R,
        ability: usize,
        object: u64,
    ) -> Option<AbilityCooldown> {
        let float = |field: u64| {
            let bits = read_u32_le(&reader.read_vec(object.checked_add(field)?, 4).ok()?, 0)?;
            Some(f32::from_bits(bits)).filter(|seconds| seconds.is_finite())
        };
        let remaining = float(self.time)?;
        let duration = float(self.duration)?;
        (remaining > 0.0 && remaining <= MAX_SECONDS && (0.0..=MAX_SECONDS).contains(&duration))
            .then_some(AbilityCooldown {
                ability,
                remaining,
                duration,
            })
    }
}

/// The displacement of the first `movss` in `code` of kind `op` with a `[reg+disp32]` operand.
fn movss(code: &[u8], op: u8) -> Option<u32> {
    (0..code.len()).find_map(|at| {
        let [0xF3, rest @ ..] = code.get(at..)? else {
            return None;
        };
        // A REX prefix, for the higher registers.
        let rest = rest
            .split_first()
            .filter(|&(&rex, _)| rex & 0xF0 == 0x40)
            .map_or(rest, |(_, rest)| rest);
        let [0x0F, found, modrm, ..] = *rest else {
            return None;
        };
        // mod 10 (disp32), no SIB
        (found == op && modrm >> 6 == 2 && modrm & 7 != 4).then(|| read_u32_le(rest, 3))?
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeReader;

    const HEAP: u64 = 0x2_0000_0000;

    #[test]
    fn finds_the_fields_the_bindings_use() {
        // call; mov rcx, rbx; movss xmm1, [rax+0x664]; call
        let get = [
            0xE8, 0, 0, 0, 0, 0x48, 0x8B, 0xCB, 0xF3, 0x0F, 0x10, 0x88, 0x64, 0x06, 0, 0, 0xE8,
        ];
        assert_eq!(movss(&get, MOVSS_LOAD), Some(0x664));
        assert_eq!(movss(&get, MOVSS_STORE), None);
        // movss [rbx+0x610], xmm0
        let set = [0xF3, 0x0F, 0x11, 0x83, 0x10, 0x06, 0, 0];
        assert_eq!(movss(&set, MOVSS_STORE), Some(0x610));
    }

    #[test]
    fn reads_a_running_cooldown() {
        let mut heap = vec![0u8; 0x1000];
        let object = HEAP + 0x100;
        let mut put = |at: u64, value: f32| {
            let at = usize::try_from(at - HEAP).expect("offset");
            heap[at..at + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(object + 0x664, 12.5);
        put(object + 0x610, 30.0);
        put(object + 0x800 + 0x610, 30.0);
        let reader = FakeReader::new(vec![(HEAP, heap)]);
        let fields = CooldownFields::at(0x664, 0x610);
        assert_eq!(
            fields.read(&reader, 1, object),
            Some(AbilityCooldown {
                ability: 1,
                remaining: 12.5,
                duration: 30.0,
            })
        );
        // Not on cooldown.
        assert_eq!(fields.read(&reader, 1, object + 0x800), None);
    }
}
