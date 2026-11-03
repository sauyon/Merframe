//! Whether the player is in a mission, and whether the game's menu is up over it. The chat
//! script follows every change of screen and keeps the first as `mIsInGameplay`: false in the
//! orbiter and relays, and from the moment a mission ends and its loading screen starts. The
//! squad panel, which the Esc menu opens beside it, is `mVisible` only while that menu is up.
//! The game hides its HUD by setting the shared `HideHud` off 0, as on the end-of-mission
//! screen, which comes up while the player is still in the mission.

use wf_mem::MemoryReader;

use crate::lua::{LuaState, LuaTable, LuaValue};

const IN_GAMEPLAY: &str = "mIsInGameplay";
/// Owned by the squad panel's script alone.
const SQUAD_PANEL: &str = "mCanMergeSquad";
const VISIBLE: &str = "mVisible";
const HIDE_HUD: &str = "HideHud";

/// `None` when the chat script is not loaded or the flag unreadable. `script` caches where the
/// chat script was last found.
pub fn in_gameplay<R: MemoryReader + ?Sized>(
    reader: &R,
    state: &LuaState,
    script: &mut Option<LuaTable>,
) -> Option<bool> {
    flag(
        reader,
        state.cached_script(reader, IN_GAMEPLAY, script)?,
        IN_GAMEPLAY,
    )
}

/// Whether the Esc menu is up. `None` when the squad panel is not loaded or the flag
/// unreadable. `script` caches where the squad panel was last found.
pub fn menu_open<R: MemoryReader + ?Sized>(
    reader: &R,
    state: &LuaState,
    script: &mut Option<LuaTable>,
) -> Option<bool> {
    flag(
        reader,
        state.cached_script(reader, SQUAD_PANEL, script)?,
        VISIBLE,
    )
}

/// Whether the game has hidden its HUD. `None` when the value is unreadable.
pub fn hud_hidden<R: MemoryReader + ?Sized>(reader: &R, state: &LuaState) -> Option<bool> {
    let hidden = state.shared(reader)?.field(reader, HIDE_HUD)?.number()?;
    Some(hidden != 0.0)
}

fn flag<R: MemoryReader + ?Sized>(reader: &R, script: LuaTable, name: &str) -> Option<bool> {
    match script.field(reader, name)? {
        LuaValue::Boolean(value) => Some(value),
        _ => None,
    }
}
