//! Ability timers the HUD counts down under each of the local player's ability icons.
//!
//! Ability scripts hand the HUD a timer through `LotusHudRedux:AddAbilityTimer(type, owner,
//! seconds, key, paused)`, one per buff they keep up; the HUD keeps, for each of the suit's
//! abilities, a list of `{key, seconds}` and takes the frame time off each one every frame
//! until it runs out. Nothing here is a fixed address or offset: the HUD is found through the
//! Lua userdata of its class, and where it keeps the abilities and their lists is read out of
//! `AddAbilityTimer`'s own code, then checked against the live object. The abilities it keeps
//! are handles to the suit's ability objects, whose types name them.

use wf_mem::{MemoryReader, read_u32_le, read_u64_le};

use crate::Result;
use crate::ability_cooldowns::{AbilityCooldown, CooldownFields};
use crate::hud_vitals::{HudVitals, Vitals};
use crate::swig::{Image, counted_arrays, indexed_arrays};
use crate::type_names::TypeNames;

const HUD_CLASS: &str = "LotusHudRedux";
const ADD_TIMER: &str = "AddAbilityTimer";
/// Writes the seconds an ability script counts down itself under its icon, as text.
const SET_TIMER: &str = "SetAbilityTimer";
/// How much of `AddAbilityTimer`'s implementation to read for its field accesses.
const IMPL_SPAN: usize = 0x400;
/// Array headers: `{pointer, u32 bytes, u32 capacity}`.
const HEADER: usize = 16;
const ABILITY: u64 = 8;
const LIST: u64 = 16;
const TIMER: u64 = 16;
const MAX_ABILITIES: u64 = 16;
const MAX_TIMERS: u64 = 32;
/// A label: a string with its text inline while short, ending in the room it has left.
const LABEL: u64 = 16;

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

/// Seconds an ability script counts down itself and has the HUD write under its icon, like
/// Grasp of Lohk's. Whole seconds, and held still while the ability is paused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityLabel {
    /// Which of the suit's abilities it belongs to, from 0, in the suit's own order.
    pub ability: usize,
    /// Seconds left, as shown.
    pub remaining: f32,
}

/// Where the HUD keeps its ability timers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityTimers {
    handle: u64,
    hud: u64,
    abilities: u64,
    lists: u64,
    names: Option<TypeNames>,
    vitals: Option<HudVitals>,
    cooldowns: Option<CooldownFields>,
    /// The HUD's array of one label per ability, which `SetAbilityTimer` writes.
    labels: Option<u64>,
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
        let names = TypeNames::locate(&image);
        if names.is_none() {
            tracing::debug!("No type name lookup in the image");
        }
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
                            names,
                            vitals: None,
                            cooldowns: None,
                            labels: None,
                        };
                        timers.read(reader).is_some().then_some(timers)
                    })
                })
            });
        let Some(found) = found else {
            tracing::debug!(
                ?counts,
                ?lists,
                "No live HUD matches its ability timer layout"
            );
            return Ok(None);
        };
        // Only once there is a HUD to read them from, as finding them reads through the image.
        let vitals = HudVitals::locate(&image);
        if vitals.is_none() {
            tracing::debug!("No health or energy readouts in the image");
        }
        let cooldowns = CooldownFields::locate(&image);
        if cooldowns.is_none() {
            tracing::debug!("No ability cooldowns in the image");
        }
        let labels = class
            .method(&image, SET_TIMER)
            .and_then(|wrapper| image.callee(wrapper))
            .and_then(|body| found.label_array(reader, &image, body));
        if labels.is_none() {
            tracing::debug!("No {HUD_CLASS}:{SET_TIMER} labels");
        }
        Ok(Some(Self {
            vitals,
            cooldowns,
            labels,
            ..found
        }))
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

    /// The names of the suit's abilities in its own order, `GeodeShardsAbility`: the ones the
    /// HUD runs timers for, so the ones the player is using, Helminth's included. `None` once
    /// the HUD is gone or any of them can't be named.
    pub fn abilities<R: MemoryReader + ?Sized>(&self, reader: &R) -> Option<Vec<String>> {
        let names = self.names?;
        self.objects(reader)?
            .into_iter()
            .map(|object| names.of_object(reader, object))
            .collect()
    }

    /// The cooldowns running on the suit's abilities. `None` once the HUD is gone.
    pub fn cooldowns<R: MemoryReader + ?Sized>(&self, reader: &R) -> Option<Vec<AbilityCooldown>> {
        let fields = self.cooldowns?;
        let objects = self.objects(reader)?;
        Some(
            objects
                .into_iter()
                .enumerate()
                .filter_map(|(ability, object)| fields.read(reader, ability, object))
                .collect(),
        )
    }

    /// Where the HUD keeps its labels: `SetAbilityTimer` formats the seconds and hands them to
    /// a function that indexes an array of them by the ability's place. It is one of the
    /// calls the formatting makes; the array is the one holding a label per ability.
    fn label_array<R: MemoryReader + ?Sized>(
        &self,
        reader: &R,
        image: &Image,
        body: u64,
    ) -> Option<u64> {
        let (_, count) = array(reader, self.hud + self.abilities, ABILITY)?;
        image
            .calls(body, IMPL_SPAN)
            .into_iter()
            .filter_map(|target| image.code(target, IMPL_SPAN))
            .flat_map(indexed_arrays)
            .map(u64::from)
            .filter(|&field| field != self.lists)
            .find(|&field| {
                array(reader, self.hud + field, LABEL).is_some_and(|(_, labels)| labels == count)
            })
    }

    /// The seconds written under each ability's icon, for the ones showing a number. `None`
    /// once the HUD is gone or the labels can't be read.
    pub fn labels<R: MemoryReader + ?Sized>(&self, reader: &R) -> Option<Vec<AbilityLabel>> {
        if reader.read_u64(self.handle).ok()? != self.hud {
            return None;
        }
        let (labels, count) = array(reader, self.hud + self.labels?, LABEL)?;
        if count > MAX_ABILITIES {
            return None;
        }
        let data = reader
            .read_vec(labels, usize::try_from(count * LABEL).ok()?)
            .ok()?;
        Some(
            data.as_chunks::<16>()
                .0
                .iter()
                .enumerate()
                .filter_map(|(ability, label)| {
                    Some(AbilityLabel {
                        ability,
                        remaining: seconds(label)?,
                    })
                })
                .collect(),
        )
    }

    /// The suit's ability objects, in its own order. `None` once the HUD is gone.
    fn objects<R: MemoryReader + ?Sized>(&self, reader: &R) -> Option<Vec<u64>> {
        if reader.read_u64(self.handle).ok()? != self.hud {
            return None;
        }
        let (abilities, count) = array(reader, self.hud + self.abilities, ABILITY)?;
        if count == 0 || count > MAX_ABILITIES {
            return None;
        }
        let handles = reader
            .read_vec(abilities, usize::try_from(count * ABILITY).ok()?)
            .ok()?;
        handles
            .as_chunks::<8>()
            .0
            .iter()
            .map(|handle| reader.read_u64(u64::from_le_bytes(*handle)).ok())
            .collect()
    }

    /// What the HUD shows of the player's health, shields, overguard and energy. `None` once
    /// the HUD is gone or while it shows none.
    pub fn vitals<R: MemoryReader + ?Sized>(&self, reader: &R) -> Option<Vitals> {
        if reader.read_u64(self.handle).ok()? != self.hud {
            return None;
        }
        self.vitals?.read(reader, self.hud)
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

/// The seconds a label shows, when it holds a number inline: the last byte is how many of
/// the 15 it has room for are left.
fn seconds(label: &[u8; 16]) -> Option<f32> {
    let len = 15usize.checked_sub(usize::from(label[15]))?;
    let text = std::str::from_utf8(&label[..len]).ok()?;
    text.trim()
        .parse::<f32>()
        .ok()
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
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
        let (heap, found) = hud_heap();
        (FakeReader::new(vec![(HEAP, heap)]), found)
    }

    fn hud_heap() -> (Vec<u8>, AbilityTimers) {
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
            names: None,
            vitals: None,
            cooldowns: None,
            labels: None,
        };
        (heap, found)
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

    fn label(text: &str) -> [u8; 16] {
        let mut label = [0u8; 16];
        label[..text.len()].copy_from_slice(text.as_bytes());
        label[15] = u8::try_from(15 - text.len()).expect("short");
        label
    }

    #[test]
    fn labels_read_as_seconds() {
        assert_eq!(seconds(&label("17")), Some(17.0));
        assert_eq!(seconds(&label("2.5")), Some(2.5));
        assert_eq!(seconds(&label("")), None);
        // Left over from a longer label, past the length.
        let mut stale = label("6");
        stale[1] = b'6';
        assert_eq!(seconds(&stale), Some(6.0));
        // A long label, kept elsewhere.
        let mut long = [0u8; 16];
        long[15] = 0x80;
        assert_eq!(seconds(&long), None);
    }

    #[test]
    fn reads_the_label_under_each_ability() {
        let (mut heap, timers) = hud_heap();
        let labels = HEAP + 0xb00;
        header(&mut heap, timers.hud + 0x458, labels, 2 * 16);
        put(&mut heap, labels, &label(""));
        put(&mut heap, labels + 16, &label("17"));
        let reader = FakeReader::new(vec![(HEAP, heap)]);
        let with = AbilityTimers {
            labels: Some(0x458),
            ..timers
        };
        assert_eq!(
            with.labels(&reader),
            Some(vec![AbilityLabel {
                ability: 1,
                remaining: 17.0
            }])
        );
        assert_eq!(timers.labels(&reader), None);
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

    #[test]
    fn names_the_suits_abilities() {
        let (mut heap, timers) = hud_heap();
        let heap = &mut heap;
        let (abilities, objects, kinds, table, pool) = (
            HEAP + 0x800,
            HEAP + 0xc00,
            HEAP + 0xe00,
            HEAP + 0xf00,
            HEAP + 0x1000,
        );
        for (index, offset) in [(0u64, 0u32), (1, 0x20)] {
            let (object, kind) = (objects + index * 0x40, kinds + index * 0x40);
            put(heap, abilities + index * 8, &(object + 0x30).to_le_bytes());
            put(heap, object + 0x30, &object.to_le_bytes());
            put(heap, object + 8, &kind.to_le_bytes());
            put(heap, kind + 0x2c, &(offset << 16 | 1).to_le_bytes());
        }
        put(heap, table, &(table + 0x10).to_le_bytes());
        put(heap, table + 0x20, &pool.to_le_bytes());
        put(heap, pool, b"GeodeShardsAbility ");
        put(heap, pool + 0x20, b"GeodeShellAbility ");
        let reader = FakeReader::new(vec![(HEAP, heap.clone())]);
        let named = AbilityTimers {
            names: Some(TypeNames::at(8, 0x2c, table)),
            ..timers
        };
        assert_eq!(
            named.abilities(&reader).as_deref(),
            Some(
                &[
                    "GeodeShardsAbility".to_owned(),
                    "GeodeShellAbility".to_owned()
                ][..]
            )
        );
        assert_eq!(timers.abilities(&reader), None);
    }
}
