//! Ability timers the HUD counts down under each of the local player's ability icons.
//!
//! Ability scripts hand the HUD a timer through `LotusHudRedux:AddAbilityTimer(type, owner,
//! seconds, key, paused)`, one per buff they keep up; the HUD keeps, for each of the suit's
//! abilities, a list of `{key, seconds}` and takes the frame time off each one every frame
//! until it runs out. Nothing here is a fixed address or offset: the HUD is found through the
//! Lua userdata of its class, and where it keeps the abilities and their lists is read out of
//! `AddAbilityTimer`'s own code, then checked against the live object.

use wf_mem::{MemoryReader, read_u32_le, read_u64_le};

use crate::Result;
use crate::swig::{Image, counted_arrays, indexed_arrays};

const HUD_CLASS: &str = "LotusHudRedux";
const ADD_TIMER: &str = "AddAbilityTimer";
/// How much of `AddAbilityTimer`'s implementation to read for its field accesses.
const IMPL_SPAN: usize = 0x400;
/// Array headers: `{pointer, u32 bytes, u32 capacity}`.
const HEADER: usize = 16;
const ABILITY: u64 = 8;
const LIST: u64 = 16;
const TIMER: u64 = 16;
const MAX_ABILITIES: u64 = 16;
const MAX_TIMERS: u64 = 32;

/// One timer the HUD runs for an ability.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityTimer {
    /// Which of the suit's abilities it belongs to, from 0, in the suit's own order.
    pub ability: usize,
    /// What the ability script told it apart by: one per buff it keeps up.
    pub key: u64,
    /// Seconds left.
    pub remaining: f32,
}

/// Where the HUD keeps its ability timers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityTimers {
    handle: u64,
    hud: u64,
    abilities: u64,
    lists: u64,
}

impl AbilityTimers {
    /// Finds the live HUD and where its timers are. `None` while no HUD is loaded, or when the
    /// game's code no longer matches what this expects.
    pub fn locate<R: MemoryReader + ?Sized>(reader: &R) -> Result<Option<Self>> {
        let image = Image::read(reader)?;
        let Some(class) = image.class(HUD_CLASS) else {
            tracing::debug!("No {HUD_CLASS} binding in the image");
            return Ok(None);
        };
        let Some(code) = class
            .method(&image, ADD_TIMER)
            .and_then(|wrapper| image.callee(wrapper))
            .and_then(|body| image.code(body, IMPL_SPAN))
        else {
            tracing::debug!("No {HUD_CLASS}:{ADD_TIMER} implementation");
            return Ok(None);
        };
        let counts = counted_arrays(code, 3);
        let lists = indexed_arrays(code);
        let found = class
            .instances(reader)?
            .into_iter()
            .find_map(|(handle, hud)| {
                counts.iter().find_map(|&abilities| {
                    lists.iter().find_map(|&list| {
                        let timers = Self {
                            handle,
                            hud,
                            abilities: u64::from(abilities),
                            lists: u64::from(list),
                        };
                        timers.read(reader).is_some().then_some(timers)
                    })
                })
            });
        if found.is_none() {
            tracing::debug!(
                ?counts,
                ?lists,
                "No live HUD matches its ability timer layout"
            );
        }
        Ok(found)
    }

    /// Every running timer, or `None` once the HUD is gone or no longer looks like one.
    pub fn read<R: MemoryReader + ?Sized>(&self, reader: &R) -> Option<Vec<AbilityTimer>> {
        if reader.read_u64(self.handle).ok()? != self.hud {
            return None;
        }
        let (abilities, count) = array(reader, self.hud + self.abilities, ABILITY)?;
        let (lists, lists_count) = array(reader, self.hud + self.lists, LIST)?;
        if count == 0 || count > MAX_ABILITIES || lists_count != count {
            return None;
        }
        reader.read_u64(abilities).ok()?;
        let headers = reader
            .read_vec(lists, usize::try_from(count * LIST).ok()?)
            .ok()?;
        let mut timers = Vec::new();
        for (ability, header) in headers.as_chunks::<HEADER>().0.iter().enumerate() {
            let (entries, len) = header_of(header, TIMER)?;
            if len > MAX_TIMERS {
                return None;
            }
            if len == 0 {
                continue;
            }
            let data = reader
                .read_vec(entries, usize::try_from(len * TIMER).ok()?)
                .ok()?;
            for entry in data.as_chunks::<16>().0 {
                let remaining = f32::from_bits(read_u32_le(entry, 8)?);
                if !remaining.is_finite() || remaining < 0.0 {
                    return None;
                }
                timers.push(AbilityTimer {
                    ability,
                    key: read_u64_le(entry, 0)?,
                    remaining,
                });
            }
        }
        Some(timers)
    }
}

/// An array header's pointer and element count, if the byte count is whole elements and
/// within capacity.
fn header_of(header: &[u8], stride: u64) -> Option<(u64, u64)> {
    let pointer = read_u64_le(header, 0)?;
    let bytes = u64::from(read_u32_le(header, 8)?);
    let capacity = u64::from(read_u32_le(header, 12)?);
    (bytes.is_multiple_of(stride) && bytes <= capacity && (bytes == 0 || pointer != 0))
        .then_some((pointer, bytes / stride))
}

fn array<R: MemoryReader + ?Sized>(reader: &R, at: u64, stride: u64) -> Option<(u64, u64)> {
    header_of(&reader.read_vec(at, HEADER).ok()?, stride)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeReader;

    const HEAP: u64 = 0x2_0000_0000;

    fn put(heap: &mut [u8], at: u64, bytes: &[u8]) {
        let at = usize::try_from(at - HEAP).expect("offset");
        heap[at..at + bytes.len()].copy_from_slice(bytes);
    }

    fn header(heap: &mut [u8], at: u64, pointer: u64, bytes: u32) {
        put(heap, at, &pointer.to_le_bytes());
        put(heap, at + 8, &bytes.to_le_bytes());
        put(heap, at + 12, &bytes.to_le_bytes());
    }

    fn hud() -> (FakeReader, AbilityTimers) {
        let mut heap = vec![0u8; 0x2000];
        let (handle, hud, abilities, lists, timers) =
            (HEAP, HEAP + 0x100, HEAP + 0x800, HEAP + 0x900, HEAP + 0xa00);
        put(&mut heap, handle, &hud.to_le_bytes());
        header(&mut heap, hud + 0xd0, abilities, 2 * 8);
        header(&mut heap, hud + 0x478, lists, 2 * 16);
        header(&mut heap, lists + 16, timers, 2 * 16);
        put(&mut heap, timers, &7u64.to_le_bytes());
        put(&mut heap, timers + 8, &90.5f32.to_le_bytes());
        put(&mut heap, timers + 16, &9u64.to_le_bytes());
        put(&mut heap, timers + 24, &12.0f32.to_le_bytes());
        let found = AbilityTimers {
            handle,
            hud,
            abilities: 0xd0,
            lists: 0x478,
        };
        (FakeReader::new(vec![(HEAP, heap)]), found)
    }

    #[test]
    fn reads_each_abilitys_timers() {
        let (reader, timers) = hud();
        let read = timers.read(&reader).expect("timers");
        assert_eq!(
            read,
            [
                AbilityTimer {
                    ability: 1,
                    key: 7,
                    remaining: 90.5
                },
                AbilityTimer {
                    ability: 1,
                    key: 9,
                    remaining: 12.0
                },
            ]
        );
    }

    #[test]
    fn a_mismatched_layout_reads_nothing() {
        let (reader, timers) = hud();
        let wrong = AbilityTimers {
            lists: 0x470,
            ..timers
        };
        assert_eq!(wrong.read(&reader), None);
        let gone = AbilityTimers {
            hud: timers.hud + 8,
            ..timers
        };
        assert_eq!(gone.read(&reader), None);
    }
}
