//! Custom ability HUDs: widgets an ability's Lua script draws for buffs it keeps up, like
//! Tribute's quiver of buffs above the weapon. Such a widget names the suit ability it
//! belongs to and keeps a list of entries each with the seconds it has left; that shape is
//! all this looks for, not any one ability.
//!
//! The ability's script describes its buffs too: a table per buff with a `uid`, its place in
//! the widget's list, and the HUD buff type it shows as, keyed by the buff's name. Only widgets
//! so described are buffs: Ivara's quiver has the same shape, but its arrows are not, and a
//! squadmate's script describing its own buffs draws no widget of the player's.
//!
//! The script drawing a widget drives it through functions it shares with every script, named
//! for the ability (`FAIRY_SetActiveSoul`), and keeps which entry the player has switched to in
//! a shared index under the same name (`fairySoulIdx`, from 0). Some widgets are drawn by a HUD
//! script apart from the ability script describing their buffs, which then shares a table of
//! buffs under that name too (`fairySoulBuffs`).

use std::collections::BTreeMap;

use wf_mem::MemoryReader;

use crate::lua::{LuaState, LuaTable, LuaValue};

const ABILITY_INDEX: &str = "mPowerSuitAbilityIndex";
/// Ends the name of a shared index.
const INDEX: &str = "Idx";
/// Ends the name of a shared table of an ability's buffs, beside its shared index.
const BUFFS: &str = "Buffs";
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

/// The ability HUDs whose entries are described as buffs, as their tables, each with the name
/// of the buff at each place in its list: described by the script drawing the HUD, else, for
/// a HUD whose ability shares a table of buffs under its name, by the one description every
/// script agrees on.
pub fn ability_huds<R: MemoryReader + ?Sized>(
    reader: &R,
    lua: &LuaState,
) -> Vec<(LuaTable, BTreeMap<usize, String>)> {
    // Each script's locals, with the buffs those locals describe.
    type Script = (Vec<LuaTable>, Vec<BTreeMap<usize, String>>);
    let scripts: Vec<Script> = lua
        .scripts(reader)
        .into_iter()
        .map(|script| {
            let locals = script.locals(reader);
            let described = locals
                .iter()
                .map(|&table| buffs(reader, table))
                .filter(|buffs| !buffs.is_empty())
                .collect();
            (locals, described)
        })
        .collect();
    let everywhere: Vec<&BTreeMap<usize, String>> = scripts
        .iter()
        .flat_map(|(_, described)| described)
        .collect();
    scripts
        .iter()
        .flat_map(|(locals, described)| {
            locals.iter().filter_map(|&table| {
                let count = ability_hud(reader, table)?.timers.len();
                let covers = |buffs: &&BTreeMap<usize, String>| {
                    (0..count).all(|index| buffs.contains_key(&index))
                };
                let names = match described.iter().find(covers) {
                    Some(names) => names,
                    None if shares_buffs(reader, lua, table) => {
                        let mut agreed = everywhere.iter().copied().filter(covers);
                        let names = agreed.next()?;
                        agreed.all(|other| other == names).then_some(names)?
                    }
                    None => return None,
                };
                Some((table, names.clone()))
            })
        })
        .collect()
}

/// Whether the ability of the HUD `table` shares a table of its buffs, named like the index
/// of the entry the player has switched to.
fn shares_buffs<R: MemoryReader + ?Sized>(reader: &R, lua: &LuaState, table: LuaTable) -> bool {
    switch_index(reader, lua, table)
        .and_then(|index| {
            let name = format!("{}{BUFFS}", index.strip_suffix(INDEX)?);
            lua.shared(reader)?.field(reader, &name)?.table()
        })
        .is_some()
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

/// `table`'s buffs, as the name of each by its place in its widget's list, if every entry
/// describes one. A script can describe more buffs than its widget lists, like ones only for
/// other game modes.
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
