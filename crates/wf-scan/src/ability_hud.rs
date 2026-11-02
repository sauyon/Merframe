//! Custom ability HUDs: widgets an ability's Lua script draws for buffs it keeps up, like
//! Tribute's quiver of buffs above the weapon. Such a widget names the suit ability it
//! belongs to and keeps a list of entries each with the seconds it has left; that shape is
//! all this looks for, not any one ability.
//!
//! The ability scripts describe their buffs too: a table per buff with a `uid`, its place in
//! the widget's list, and the HUD buff type it shows as, keyed by the buff's name.
//!
//! The script drives its widget through functions it shares with every script, named for the
//! ability (`FAIRY_SetActiveSoul`), and keeps which entry the player has switched to in a
//! shared index under the same name (`fairySoulIdx`, from 0).

use std::collections::BTreeMap;

use wf_mem::MemoryReader;

use crate::lua::{LuaState, LuaTable, LuaValue};

const ABILITY_INDEX: &str = "mPowerSuitAbilityIndex";
/// Ends the name of a shared index.
const INDEX: &str = "Idx";
const TIMER: &str = "Timer";
const BUFF_ID: &str = "uid";
const BUFF_TYPE: &str = "abilityHudBuffType";

/// What an ability HUD shows now.
#[derive(Debug, Clone, PartialEq)]
pub struct AbilityHud {
    /// The suit ability, from 0, it belongs to.
    pub ability: usize,
    /// Seconds left on each entry, 0 for ones not running.
    pub timers: Vec<f32>,
}

/// The scripts drawing ability HUDs, as their tables.
pub fn ability_huds<R: MemoryReader + ?Sized>(reader: &R, lua: &LuaState) -> Vec<LuaTable> {
    lua.scripts(reader)
        .into_iter()
        .flat_map(|script| script.locals(reader))
        .filter(|table| ability_hud(reader, *table).is_some())
        .collect()
}

/// What the HUD `table` shows. `None` once the table no longer looks like an ability HUD.
pub fn ability_hud<R: MemoryReader + ?Sized>(reader: &R, table: LuaTable) -> Option<AbilityHud> {
    let ability = usize::try_from(table.field(reader, ABILITY_INDEX)?.whole()?).ok()?;
    let timers: Vec<f32> = table
        .entries(reader)
        .into_iter()
        .filter_map(|(_, value)| value.table())
        .map(|list| timers(reader, list))
        .find(|found| !found.is_empty())?;
    Some(AbilityHud { ability, timers })
}

/// The name of the shared index holding which entry of the HUD `table` the player has
/// switched to: the one named like the shared functions that drive the HUD.
pub fn switch_index<R: MemoryReader + ?Sized>(
    reader: &R,
    lua: &LuaState,
    table: LuaTable,
) -> Option<String> {
    let entries = lua.shared(reader)?.entries(reader);
    let names: Vec<String> = entries
        .iter()
        .filter(|(_, value)| {
            value
                .clone()
                .function()
                .is_some_and(|function| function.upvalues(reader).contains(&LuaValue::Table(table)))
        })
        .filter_map(|(key, _)| Some(key.split_once('_')?.0.to_lowercase()))
        .collect();
    entries
        .into_iter()
        .filter(|(key, value)| key.ends_with(INDEX) && matches!(value, LuaValue::Number(_)))
        .map(|(key, _)| key)
        .find(|key| {
            let key = key.to_lowercase();
            names.iter().any(|name| key.starts_with(name.as_str()))
        })
}

/// The entry, of `count`, the shared index `name` says the player has switched to.
pub fn switched_to<R: MemoryReader + ?Sized>(
    reader: &R,
    lua: &LuaState,
    name: &str,
    count: usize,
) -> Option<usize> {
    let index = lua.shared(reader)?.field(reader, name)?.whole()?;
    usize::try_from(index).ok().filter(|&index| index < count)
}

/// Each entry's `Timer` in `list`, if every entry has one.
fn timers<R: MemoryReader + ?Sized>(reader: &R, list: LuaTable) -> Vec<f32> {
    let entries: Vec<LuaValue> = list.items(reader);
    let count = entries.len();
    let found: Vec<f32> = entries
        .into_iter()
        .filter_map(|entry| {
            entry
                .table()?
                .field(reader, TIMER)
                .and_then(LuaValue::number)
                .filter(|seconds| seconds.is_finite())
        })
        .collect();
    if found.len() == count {
        found
    } else {
        Vec::new()
    }
}

/// The buffs each loaded ability script describes, as the name of each by its place in its
/// widget's list. A script can describe more buffs than its widget lists, like ones only
/// for other game modes.
pub fn ability_buffs<R: MemoryReader + ?Sized>(
    reader: &R,
    lua: &LuaState,
) -> Vec<BTreeMap<usize, String>> {
    let mut found: Vec<BTreeMap<usize, String>> = lua
        .scripts(reader)
        .into_iter()
        .flat_map(|script| script.locals(reader))
        .map(|table| buffs(reader, table))
        .filter(|buffs| !buffs.is_empty())
        .collect();
    found.sort();
    found.dedup();
    found
}

/// `table`'s buffs, if every entry describes one.
fn buffs<R: MemoryReader + ?Sized>(reader: &R, table: LuaTable) -> BTreeMap<usize, String> {
    let entries = table.entries(reader);
    let count = entries.len();
    let found: BTreeMap<usize, String> = entries
        .into_iter()
        .filter_map(|(name, value)| {
            let buff = value.table()?;
            buff.field(reader, BUFF_TYPE)?.userdata()?;
            let id = usize::try_from(buff.field(reader, BUFF_ID)?.whole()?).ok()?;
            Some((id, name))
        })
        .collect();
    if found.len() == count {
        found
    } else {
        BTreeMap::new()
    }
}
