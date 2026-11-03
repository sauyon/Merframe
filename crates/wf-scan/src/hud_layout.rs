//! Where the HUD puts the ability dots under the reticle, and how large it draws them. The HUD
//! script lays its clips out on a fixed stage with an anchor manager: each clip keeps the
//! position it was authored at and the one the manager moved it to after pulling the edges in
//! by the HUD border padding, edge-anchored clips moving and centred ones staying put. The
//! same script keeps the player's HUD scale in a table of its own.

use wf_mem::MemoryReader;

use crate::lua::{LuaState, LuaTable, LuaValue};

/// Owned by the HUD script, which also drives the buff tray.
const HUD_SCRIPT: &str = "OnProfileSaved";
const HUD_SCALE: &str = "HudScale";
const LIST: &str = "list";
const STAGE_WIDTH: &str = "stageWidth";
const STAGE_HEIGHT: &str = "stageHeight";
const CLIP: &str = "clip";
const DOTS: &str = "AbilityDots";
const X: &str = "NewX";
const Y: &str = "NewY";

/// The ability dots' clip, in stage units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DotsLayout {
    pub stage_width: f32,
    pub stage_height: f32,
    /// Where the anchor manager put the dots' clip.
    pub x: f32,
    pub y: f32,
    /// The player's HUD scale, 1 at the default size.
    pub hud_scale: f32,
}

/// Where the HUD script's tables were last found.
#[derive(Debug, Default)]
pub struct HudTables {
    anchors: Option<LuaTable>,
    scale: Option<LuaTable>,
}

/// `None` while the HUD is not loaded or its layout unreadable.
pub fn ability_dots<R: MemoryReader + ?Sized>(
    reader: &R,
    state: &LuaState,
    tables: &mut HudTables,
) -> Option<DotsLayout> {
    let live = |table: Option<LuaTable>, field| {
        table.is_some_and(|table: LuaTable| table.field(reader, field).is_some())
    };
    if !live(tables.anchors, STAGE_HEIGHT) || !live(tables.scale, HUD_SCALE) {
        *tables = find(reader, state).unwrap_or_default();
    }
    let (anchors, scale) = (tables.anchors?, tables.scale?);
    let dots = anchors
        .field(reader, LIST)
        .and_then(LuaValue::table)?
        .tables(reader)
        .into_iter()
        .find(|clip| clip.text(reader, CLIP).as_deref() == Some(DOTS))?;
    Some(DotsLayout {
        stage_width: number(reader, anchors, STAGE_WIDTH)?,
        stage_height: number(reader, anchors, STAGE_HEIGHT)?,
        x: number(reader, dots, X)?,
        y: number(reader, dots, Y)?,
        hud_scale: number(reader, scale, HUD_SCALE)?,
    })
}

/// The anchor manager and the scale table among the locals of a script owning
/// [`HUD_SCRIPT`]; other scripts own it too.
fn find<R: MemoryReader + ?Sized>(reader: &R, state: &LuaState) -> Option<HudTables> {
    state
        .scripts(reader)
        .into_iter()
        .filter(|script| script.field(reader, HUD_SCRIPT).is_some())
        .find_map(|script| {
            let locals = script.locals(reader);
            let owning = |field| {
                locals
                    .iter()
                    .copied()
                    .find(|table| table.field(reader, field).is_some())
            };
            Some(HudTables {
                anchors: Some(owning(STAGE_HEIGHT)?),
                scale: Some(owning(HUD_SCALE)?),
            })
        })
}

fn number<R: MemoryReader + ?Sized>(reader: &R, table: LuaTable, name: &str) -> Option<f32> {
    table
        .field(reader, name)?
        .number()
        .filter(|n| n.is_finite())
}
