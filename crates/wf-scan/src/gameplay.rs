//! Whether the player is in a mission. The chat script follows every change of screen and keeps
//! it as `mIsInGameplay`: false in the orbiter and relays, and from the moment a mission ends
//! and its loading screen starts.

use wf_mem::MemoryReader;

use crate::lua::{LuaState, LuaTable, LuaValue};

const IN_GAMEPLAY: &str = "mIsInGameplay";

/// `None` when the chat script is not loaded or the flag unreadable. `script` caches where the
/// chat script was last found.
pub fn in_gameplay<R: MemoryReader + ?Sized>(
    reader: &R,
    state: &LuaState,
    script: &mut Option<LuaTable>,
) -> Option<bool> {
    let script = state.cached_script(reader, IN_GAMEPLAY, script)?;
    match script.field(reader, IN_GAMEPLAY)? {
        LuaValue::Boolean(playing) => Some(playing),
        _ => None,
    }
}
