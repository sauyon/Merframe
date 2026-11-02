use wf_mem::MemoryReader;

use crate::lua::{LuaState, LuaTable, LuaValue};

const HUD_SCRIPT: &str = "OnProfileSaved";
const LIST_PREFIX: &str = "mPrefix";
const SELF_LIST: &str = "Grid(HealthAndShield.StatusEffectItem)::";
const ELEMENTS: &str = "mElements";
const CHANGE: &str = "Change";
const NAME_TAG: &str = "StatNameTag";
const BUFF_TYPE: &str = "mBuffType";
const BUFF_DATA: &str = "mBuffData";
const CHANGE_DATA: &str = "BuffData";
const INITIAL_DATA: &str = "mInitialBuffData";
const IS_DEBUFF: &str = "mIsDebuff";
const PAUSED: &str = "mEffectIsPaused";
const ID: &str = "Id";

/// `mBuffType` of an effect whose data is seconds remaining.
pub const TIMED: u32 = 1;

/// One icon in the local player's HUD buff tray.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusEffect {
    pub id: u32,
    /// Localization key of the effect's name, e.g. `/Lotus/Language/Mods/ComboCritChanceMod`.
    pub name_tag: String,
    pub buff_type: u32,
    /// Seconds remaining for [`TIMED`] effects; a count or stack size otherwise.
    pub data: f32,
    /// The latest value pushed by the game, which the HUD animates `data` towards.
    pub pending_data: Option<f32>,
    /// `data` when the effect was applied: a timed effect's full duration.
    pub initial_data: f32,
    pub is_debuff: bool,
    pub paused: bool,
}

impl StatusEffect {
    pub fn is_timed(&self) -> bool {
        self.buff_type == TIMED
    }
}

fn boolean<R: MemoryReader + ?Sized>(reader: &R, table: LuaTable, name: &str) -> bool {
    matches!(table.field(reader, name), Some(LuaValue::Boolean(true)))
}

fn effect<R: MemoryReader + ?Sized>(reader: &R, element: LuaTable) -> Option<StatusEffect> {
    let change = element.field(reader, CHANGE).and_then(LuaValue::table);
    Some(StatusEffect {
        id: element.field(reader, ID)?.whole()?,
        name_tag: change
            .and_then(|change| change.text(reader, NAME_TAG))
            .unwrap_or_default(),
        buff_type: element.field(reader, BUFF_TYPE)?.whole()?,
        data: element.field(reader, BUFF_DATA)?.number()?,
        pending_data: change
            .and_then(|change| change.field(reader, CHANGE_DATA))
            .and_then(LuaValue::number),
        initial_data: element.field(reader, INITIAL_DATA)?.number()?,
        is_debuff: boolean(reader, element, IS_DEBUFF),
        paused: boolean(reader, element, PAUSED),
    })
}

/// The HUD list behind the local player's buff tray.
pub fn status_list<R: MemoryReader + ?Sized>(reader: &R, state: &LuaState) -> Option<LuaTable> {
    state
        .scripts(reader)
        .into_iter()
        .filter(|script| script.field(reader, HUD_SCRIPT).is_some())
        .flat_map(|script| script.locals(reader))
        .find(|table| table.text(reader, LIST_PREFIX).as_deref() == Some(SELF_LIST))
}

pub fn status_effects<R: MemoryReader + ?Sized>(reader: &R, list: LuaTable) -> Vec<StatusEffect> {
    list.field(reader, ELEMENTS)
        .and_then(LuaValue::table)
        .map(|elements| elements.tables(reader))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|element| effect(reader, element))
        .collect()
}
