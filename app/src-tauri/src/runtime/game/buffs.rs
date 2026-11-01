//! Polls the local player's HUD buff tray and feeds the buff ring overlay.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Runtime};
use tracing::debug;
use wf_mem::MemoryReader;
use wf_scan::{LuaState, LuaTable, StatusEffect, status_effects, status_list};

use super::ticks;
use crate::overlay::{self, BuffTimer};
use crate::runtime::attach::{Attached, reattach};
use crate::runtime::blocking;
use crate::state::{AppState, lock, read};

const BUFF_TICK: Duration = Duration::from_millis(100);
const IDLE_TICK: Duration = Duration::from_secs(2);
/// The tray is rebuilt between missions; while it reads empty, look it up again this often.
const SEARCH_EVERY: Duration = Duration::from_secs(1);
/// Seconds an expired buff stays on the ring, so the drop is noticed.
const LINGER: f64 = 3.0;
/// Remaining time jumping up by more than this means the buff was reapplied.
const REFRESH_JUMP: f64 = 0.5;
/// The ring counts down on its own; only resend when the end time moved more than this. The
/// estimate jitters by a few hundred ms between polls.
const DRIFT_MS: f64 = 1000.0;
/// How often the equipped warframe's abilities are looked up again from the inventory.
const LOADOUT_EVERY: Duration = Duration::from_secs(5);

pub(crate) async fn buffs_task<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    let mut tick = ticks(BUFF_TICK);
    let mut game: Option<Attached> = None;
    let mut tray = Tray::default();
    let mut ticking = Ticking::default();
    let mut tracker = Tracker::new(LINGER);
    let mut sent: Vec<BuffTimer> = Vec::new();
    let started = Instant::now();
    let mut seen = (None, 0);
    let mut abilities: Vec<String> = Vec::new();
    let mut loadout_read: Option<Instant> = None;
    loop {
        tick.tick().await;
        if loadout_read.is_none_or(|at| at.elapsed() >= LOADOUT_EVERY) {
            loadout_read = Some(Instant::now());
            let owned = Arc::clone(&state);
            let equipped = blocking("read the equipped abilities", move || {
                lock(&owned.core).equipped_abilities()
            })
            .await
            .flatten()
            .unwrap_or_default();
            if equipped != abilities {
                debug!(?equipped, "Buff ring slots");
                abilities = equipped;
            }
        }
        let wanted = read(&state.status).pid.is_some()
            && overlay::enabled(&read(&state.settings), overlay::Kind::Buffs);
        let effects = if wanted {
            let owned = Arc::clone(&state);
            let cached = game.take();
            let mut moved = tray;
            let attempt = blocking("read the buff tray", move || {
                let attached = reattach(&owned, cached)?;
                let effects = moved.read(attached.reader.as_ref(), &attached.lua);
                anyhow::Ok((attached, moved, effects))
            })
            .await;
            match attempt {
                Some(Ok((attached, read, effects))) => {
                    game = Some(attached);
                    tray = read;
                    Some(effects)
                }
                Some(Err(error)) => {
                    debug!(?error, "Buff tray not read");
                    None
                }
                None => None,
            }
        } else {
            None
        };

        let now = started.elapsed().as_secs_f64();
        let readings = effects.as_deref().map(|e| ticking.timed(e)).unwrap_or_default();
        let counts = (effects.as_ref().map(Vec::len), readings.len());
        if counts != seen {
            debug!(wanted, effects = counts.0, readings = counts.1, "Buff tray read");
            seen = counts;
        }
        tracker.update(&readings, now);
        let timers = timers(&tracker.shown(), unix_ms(), &abilities);
        if !same(&timers, &sent) {
            overlay::on_buffs(&app, &state, timers.clone());
            sent = timers;
        }

        if effects.is_none() {
            game = None;
            tray = Tray::default();
            tokio::time::sleep(IDLE_TICK).await;
        }
    }
}

fn unix_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64() * 1000.0)
}

/// Where the tray lives in the Lua state, and when it was last looked up.
#[derive(Default, Clone, Copy)]
struct Tray {
    list: Option<LuaTable>,
    searched: Option<Instant>,
}

impl Tray {
    fn read(&mut self, reader: &dyn MemoryReader, lua: &LuaState) -> Vec<StatusEffect> {
        let mut effects = self
            .list
            .map(|list| status_effects(reader, list))
            .unwrap_or_default();
        if effects.is_empty() && self.searched.is_none_or(|at| at.elapsed() >= SEARCH_EVERY) {
            self.searched = Some(Instant::now());
            self.list = status_list(reader, lua);
            effects = self
                .list
                .map(|list| status_effects(reader, list))
                .unwrap_or_default();
        }
        effects
    }
}

fn timers(shown: &[Shown], now_ms: f64, abilities: &[String]) -> Vec<BuffTimer> {
    shown
        .iter()
        .map(|buff| BuffTimer {
            id: buff.id.clone(),
            label: buff.label.clone(),
            slot: slot(&buff.id, abilities),
            total: buff.total,
            ends_at: buff.remaining.map(|left| (now_ms + left * 1000.0).round()),
        })
        .collect()
}

/// The ability slot, counted from 1, whose ability the buff comes from. Buff names share
/// their ability's stem: `.../Suits/FairyDustAbilityName` comes from `.../FairyDustAbility`.
fn slot(id: &str, abilities: &[String]) -> Option<u8> {
    let tag = id.rsplit_once('#').map_or(id, |(tag, _)| tag);
    let key = tag.rsplit('/').next()?;
    let (index, _) = abilities
        .iter()
        .enumerate()
        .filter_map(|(index, ability)| Some((index, ability.rsplit('/').next()?)))
        .filter(|(_, stem)| key.starts_with(stem))
        .max_by_key(|(_, stem)| stem.len())?;
    u8::try_from(index + 1).ok()
}

fn same(now: &[BuffTimer], sent: &[BuffTimer]) -> bool {
    now.len() == sent.len()
        && now.iter().zip(sent).all(|(a, b)| {
            a.id == b.id
                && a.label == b.label
                && a.slot == b.slot
                && (a.total - b.total).abs() < REFRESH_JUMP
                && match (a.ends_at, b.ends_at) {
                    (Some(x), Some(y)) => (x - y).abs() < DRIFT_MS,
                    (None, None) => true,
                    _ => false,
                }
        })
}

#[derive(Debug, Clone, PartialEq)]
struct Reading {
    id: String,
    label: String,
    remaining: f64,
}

/// Which effects count down. Learned from the data rather than from type ids: an effect is
/// timed once its value has been seen falling.
#[derive(Default)]
struct Ticking {
    last: HashMap<u32, f32>,
    timed: HashSet<u32>,
}

impl Ticking {
    fn timed(&mut self, effects: &[StatusEffect]) -> Vec<Reading> {
        for effect in effects {
            if self
                .last
                .get(&effect.id)
                .is_some_and(|&prev| effect.data < prev)
            {
                self.timed.insert(effect.id);
            }
        }
        self.last = effects.iter().map(|e| (e.id, e.data)).collect();
        self.timed.retain(|id| self.last.contains_key(id));
        effects
            .iter()
            .filter(|e| !e.is_debuff && cast(&e.name_tag) && self.timed.contains(&e.id))
            .map(|e| Reading {
                id: format!("{}#{}", e.name_tag, e.id),
                label: label(&e.name_tag),
                remaining: f64::from(e.data),
            })
            .collect()
    }
}

/// Effects the player keeps up by casting: ability buffs and arcanes that trigger on casts.
/// Mod and weapon procs come and go on their own, so they would only crowd the ring.
fn cast(tag: &str) -> bool {
    ["/Language/Suits/", "/Language/Arcanes/"]
        .iter()
        .any(|kind| tag.contains(kind))
}

/// A readable label from a localization key: `/Lotus/Language/Suits/FairyDustAbilityName`
/// becomes `Fairy Dust`.
fn label(tag: &str) -> String {
    let last = tag.rsplit('/').next().unwrap_or(tag);
    let trimmed = ["AbilityName", "Name", "Ability", "Mod"]
        .iter()
        .fold(last, |s, suffix| s.strip_suffix(suffix).unwrap_or(s));
    let mut out = String::new();
    let mut prev_lower = false;
    for c in trimmed.chars() {
        if c.is_uppercase() && prev_lower {
            out.push(' ');
        }
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
        out.push(c);
    }
    if out.is_empty() { tag.to_owned() } else { out }
}

#[derive(Debug, Clone, PartialEq)]
struct Shown {
    id: String,
    label: String,
    /// `None` once the buff has dropped off.
    remaining: Option<f64>,
    total: f64,
}

struct Entry {
    shown: Shown,
    gone_at: Option<f64>,
}

/// Turns per-poll readings into what the ring shows. The game only tells us time remaining,
/// so a buff's full length is the largest remaining value seen since it was last applied.
struct Tracker {
    entries: Vec<Entry>,
    linger: f64,
}

impl Tracker {
    fn new(linger: f64) -> Self {
        Self {
            entries: Vec::new(),
            linger,
        }
    }

    fn update(&mut self, readings: &[Reading], now: f64) {
        for reading in readings {
            match self.entries.iter_mut().find(|e| e.shown.id == reading.id) {
                Some(entry) => {
                    let shown = &mut entry.shown;
                    let refreshed = shown
                        .remaining
                        .is_none_or(|old| reading.remaining > old + REFRESH_JUMP);
                    shown.total = if refreshed {
                        reading.remaining
                    } else {
                        shown.total.max(reading.remaining)
                    };
                    shown.remaining = Some(reading.remaining);
                    shown.label.clone_from(&reading.label);
                    entry.gone_at = None;
                }
                None => self.entries.push(Entry {
                    shown: Shown {
                        id: reading.id.clone(),
                        label: reading.label.clone(),
                        remaining: Some(reading.remaining),
                        total: reading.remaining,
                    },
                    gone_at: None,
                }),
            }
        }
        for entry in &mut self.entries {
            if entry.shown.remaining.is_some() && !readings.iter().any(|r| r.id == entry.shown.id)
            {
                entry.shown.remaining = None;
                entry.gone_at = Some(now);
            }
        }
        let linger = self.linger;
        self.entries
            .retain(|e| e.gone_at.is_none_or(|at| now - at < linger));
    }

    fn shown(&self) -> Vec<Shown> {
        self.entries.iter().map(|e| e.shown.clone()).collect()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "values are copied, never computed")]
mod tests {
    use super::*;

    fn r(id: &str, remaining: f64) -> Reading {
        Reading {
            id: id.into(),
            label: id.into(),
            remaining,
        }
    }

    fn effect(id: u32, data: f32) -> StatusEffect {
        StatusEffect {
            id,
            name_tag: format!("/Lotus/Language/Suits/E{id}AbilityName"),
            buff_type: 0,
            data,
            pending_data: None,
            initial_data: data,
            is_debuff: false,
            paused: false,
        }
    }

    #[test]
    fn total_is_first_seen_and_survives_countdown() {
        let mut t = Tracker::new(3.0);
        t.update(&[r("lash", 60.0)], 0.0);
        t.update(&[r("lash", 50.0)], 10.0);
        let s = t.shown();
        assert_eq!(s[0].total, 60.0);
        assert_eq!(s[0].remaining, Some(50.0));
    }

    #[test]
    fn recast_resets_total() {
        let mut t = Tracker::new(3.0);
        t.update(&[r("lash", 60.0)], 0.0);
        t.update(&[r("lash", 10.0)], 50.0);
        t.update(&[r("lash", 75.0)], 51.0);
        assert_eq!(t.shown()[0].total, 75.0);
    }

    #[test]
    fn expired_lingers_then_drops() {
        let mut t = Tracker::new(3.0);
        t.update(&[r("a", 5.0), r("b", 5.0)], 0.0);
        t.update(&[r("b", 4.0)], 1.0);
        let s = t.shown();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].remaining, None);
        t.update(&[r("b", 1.0)], 4.0);
        assert_eq!(t.shown().len(), 1);
    }

    #[test]
    fn order_is_first_seen() {
        let mut t = Tracker::new(3.0);
        t.update(&[r("b", 5.0)], 0.0);
        t.update(&[r("a", 5.0), r("b", 4.0)], 1.0);
        let labels: Vec<_> = t.shown().into_iter().map(|s| s.label).collect();
        assert_eq!(labels, ["b", "a"]);
    }

    #[test]
    fn labels_drop_paths_and_suffixes() {
        assert_eq!(
            label("/Lotus/Language/Suits/FairyDustAbilityName"),
            "Fairy Dust"
        );
        assert_eq!(
            label("/Lotus/Language/Items/FairyPassiveBuff"),
            "Fairy Passive Buff"
        );
        assert_eq!(label(""), "");
    }

    #[test]
    fn mod_procs_are_left_off() {
        let mut t = Ticking::default();
        let mut procs = effect(3, 10.0);
        procs.name_tag = "/Lotus/Language/Mods/WeaponMeleeStatusChanceSPMod".into();
        let mut sculptor = effect(4, 10.0);
        sculptor.name_tag = "/Lotus/Language/Arcanes/AbilityDurationOnCastName".into();
        t.timed(&[procs.clone(), sculptor.clone()]);
        procs.data = 9.0;
        sculptor.data = 9.0;
        let shown = t.timed(&[procs, sculptor]);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].label, "Ability Duration On Cast");
    }

    #[test]
    fn only_effects_seen_falling_are_shown() {
        let mut t = Ticking::default();
        assert!(t.timed(&[effect(1, 10.0), effect(2, 3.0)]).is_empty());
        let shown = t.timed(&[effect(1, 9.9), effect(2, 3.0)]);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].label, "E1");
    }

    #[test]
    fn buffs_take_their_ability_slot() {
        let titania = [
            "/Lotus/Powersuits/Fairy/Abilities/FairyDustAbility",
            "/Lotus/Powersuits/Fairy/Abilities/FairySoulAbility",
            "/Lotus/Powersuits/BrokenFrame/Abilities/BrokenRotAbility",
            "/Lotus/Powersuits/Fairy/Abilities/FairyFlightAbility",
        ]
        .map(String::from);
        let slot_of = |tag: &str| slot(&format!("{tag}#7"), &titania);
        assert_eq!(slot_of("/Lotus/Language/Suits/FairyDustAbilityName"), Some(1));
        assert_eq!(slot_of("/Lotus/Language/Suits/BrokenRotAbilityName"), Some(3));
        assert_eq!(
            slot_of("/Lotus/Language/Suits/FairyFlightAbilityAugment1Name"),
            Some(4)
        );
        assert_eq!(slot_of("/Lotus/Language/Arcanes/AbilityDurationOnCastName"), None);
    }

    #[test]
    fn countdown_is_not_resent_every_tick() {
        let shown = [Shown {
            id: "a".into(),
            label: "A".into(),
            remaining: Some(10.0),
            total: 20.0,
        }];
        let first = timers(&shown, 0.0, &[]);
        let later = timers(
            &[Shown {
                remaining: Some(9.9),
                ..shown[0].clone()
            }],
            100.0,
            &[],
        );
        assert!(same(&later, &first));
        let recast = timers(
            &[Shown {
                remaining: Some(20.0),
                ..shown[0].clone()
            }],
            100.0,
            &[],
        );
        assert!(!same(&recast, &first));
    }
}
