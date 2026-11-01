//! Polls the local player's buffs from game memory and feeds the buff strip overlay.
//!
//! Buffs come from more than one place, each a [`BuffSource`]: the HUD's buff tray, and the
//! timers the HUD counts down under the ability icons, which is where abilities like Tribute
//! keep theirs.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Runtime};
use tracing::debug;
use wf_mem::MemoryReader;
use wf_scan::{AbilityTimers, LuaState, LuaTable, StatusEffect, status_effects, status_list};

use super::ticks;
use crate::overlay::{self, BuffTimer};
use crate::runtime::attach::{Attached, reattach};
use crate::runtime::blocking;
use crate::state::{AppState, lock, read};

const BUFF_TICK: Duration = Duration::from_millis(100);
const IDLE_TICK: Duration = Duration::from_secs(2);
/// The tray is rebuilt between missions; while it reads empty, look it up again this often.
const SEARCH_EVERY: Duration = Duration::from_secs(1);
/// The old tray can linger in memory with a few frozen entries, so it never reads empty;
/// even while it reads, look it up again this often.
const RECHECK_EVERY: Duration = Duration::from_secs(5);
/// Seconds an expired buff stays on the strip, so the drop is noticed.
const LINGER: f64 = 3.0;
/// Remaining time jumping up by more than this means the buff was reapplied.
const REFRESH_JUMP: f64 = 0.5;
/// The strip counts down on its own; only resend when the end time moved more than this. The
/// estimate jitters by a few hundred ms between polls.
const DRIFT_MS: f64 = 1000.0;
/// Ability timers live in the HUD, which is rebuilt between missions. Finding it scans the
/// whole heap for several seconds, so it runs on its own thread, and while the HUD can't be
/// read it is looked for again only this often, doubling up to [`HUD_SEARCH_MAX`] each time
/// it isn't found, as in menus without one.
const HUD_SEARCH_EVERY: Duration = Duration::from_secs(15);
const HUD_SEARCH_MAX: Duration = Duration::from_secs(120);
/// How often the equipped warframe's abilities are looked up again from the inventory.
const LOADOUT_EVERY: Duration = Duration::from_secs(5);

pub(crate) async fn buffs_task<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    let mut tick = ticks(BUFF_TICK);
    let mut game: Option<Attached> = None;
    let mut sources = buff_sources();
    let mut tracker = Tracker::new(LINGER);
    let mut sent: Vec<BuffTimer> = Vec::new();
    let started = Instant::now();
    let mut seen: Option<Vec<usize>> = None;
    let mut abilities: Vec<String> = Vec::new();
    let mut icons: Vec<(String, String)> = Vec::new();
    let mut loadout_read: Option<Instant> = None;
    loop {
        tick.tick().await;
        if loadout_read.is_none_or(|at| at.elapsed() >= LOADOUT_EVERY) {
            loadout_read = Some(Instant::now());
            let owned = Arc::clone(&state);
            let looked_up = blocking("read the equipped abilities", move || {
                let core = lock(&owned.core);
                (core.equipped_abilities(), core.buff_icons())
            })
            .await;
            let (equipped, known) = looked_up.unwrap_or_default();
            let equipped = equipped.unwrap_or_default();
            if !known.is_empty() {
                icons = known;
            }
            if equipped != abilities {
                debug!(?equipped, "Buff strip slots");
                abilities = equipped;
            }
        }
        let wanted = read(&state.status).pid.is_some()
            && overlay::enabled(&read(&state.settings), overlay::Kind::Buffs);
        let readings = if wanted {
            let owned = Arc::clone(&state);
            let cached = game.take();
            let mut moved = std::mem::take(&mut sources);
            let attempt = blocking("read buffs", move || {
                let attached = reattach(&owned, cached)?;
                let readings: Vec<Vec<Reading>> = moved
                    .iter_mut()
                    .map(|source| source.read(&attached.reader, &attached.lua))
                    .collect();
                anyhow::Ok((attached, moved, readings))
            })
            .await;
            match attempt {
                Some(Ok((attached, read, readings))) => {
                    game = Some(attached);
                    sources = read;
                    Some(readings)
                }
                Some(Err(error)) => {
                    debug!(?error, "Buffs not read");
                    None
                }
                None => None,
            }
        } else {
            None
        };

        let now = started.elapsed().as_secs_f64();
        let counts = readings
            .as_ref()
            .map(|by_source| by_source.iter().map(Vec::len).collect::<Vec<_>>());
        if counts != seen {
            debug!(wanted, ?counts, "Buffs read");
            seen.clone_from(&counts);
        }
        let idle = readings.is_none();
        let readings = merge(readings.unwrap_or_default(), &abilities);
        tracker.update(&readings, now);
        let timers = timers(&tracker.shown(), unix_ms(), &icons);
        if !same(&timers, &sent) {
            overlay::on_buffs(&app, &state, timers.clone());
            sent = timers;
        }

        if idle {
            game = None;
            sources = buff_sources();
            tokio::time::sleep(IDLE_TICK).await;
        }
    }
}

fn unix_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64() * 1000.0)
}

/// Somewhere in game memory that running buffs can be read from.
trait BuffSource: Send {
    /// The buffs running now; empty while the source can't be read.
    fn read(&mut self, reader: &Arc<dyn MemoryReader>, lua: &LuaState) -> Vec<Reading>;
}

fn buff_sources() -> Vec<Box<dyn BuffSource>> {
    vec![
        Box::new(TraySource::default()),
        Box::new(AbilityTimerSource::default()),
    ]
}

/// The HUD's buff tray: effects from mods, arcanes and most abilities, by name.
#[derive(Default)]
struct TraySource {
    tray: Tray,
    ticking: Ticking,
}

impl BuffSource for TraySource {
    fn read(&mut self, reader: &Arc<dyn MemoryReader>, lua: &LuaState) -> Vec<Reading> {
        let effects = self.tray.read(reader.as_ref(), lua);
        self.ticking.timed(&effects)
    }
}

/// The timers the HUD runs under each ability icon, one per buff the ability keeps up. These
/// carry no names, only which ability they belong to, so they take that ability's slot.
#[derive(Default)]
struct AbilityTimerSource {
    timers: Option<AbilityTimers>,
    searched: Option<Instant>,
    search: Option<mpsc::Receiver<Option<AbilityTimers>>>,
    /// Searches in a row that found nothing.
    misses: u32,
}

impl AbilityTimerSource {
    fn search_every(&self) -> Duration {
        HUD_SEARCH_EVERY
            .saturating_mul(1 << self.misses.min(3))
            .min(HUD_SEARCH_MAX)
    }

    /// Starts looking for the HUD in the background, unless a search is running or one ran
    /// too recently.
    fn search(&mut self, reader: &Arc<dyn MemoryReader>) {
        if self.search.is_some()
            || self
                .searched
                .is_some_and(|at| at.elapsed() < self.search_every())
        {
            return;
        }
        self.searched = Some(Instant::now());
        let (found, search) = mpsc::channel();
        let reader = Arc::clone(reader);
        let spawned = std::thread::Builder::new()
            .name("locate ability timers".into())
            .spawn(move || {
                let located = AbilityTimers::locate(reader.as_ref()).unwrap_or_else(|error| {
                    debug!(%error, "Ability timers not located");
                    None
                });
                debug!(?located, "Ability timers searched for");
                let _ = found.send(located);
            });
        match spawned {
            Ok(_) => self.search = Some(search),
            Err(error) => debug!(%error, "Ability timer search not started"),
        }
    }
}

impl BuffSource for AbilityTimerSource {
    fn read(&mut self, reader: &Arc<dyn MemoryReader>, _lua: &LuaState) -> Vec<Reading> {
        if let Some(search) = &self.search {
            match search.try_recv() {
                Ok(found) => {
                    self.misses = if found.is_some() { 0 } else { self.misses + 1 };
                    self.timers = found;
                    self.search = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.search = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let read = self.timers.and_then(|timers| timers.read(reader.as_ref()));
        if read.is_none() {
            self.search(reader);
        }
        read.unwrap_or_default()
            .into_iter()
            .map(|timer| Reading {
                id: format!("ability{}:{:x}", timer.ability, timer.key),
                label: String::new(),
                slot: u8::try_from(timer.ability + 1).ok(),
                remaining: f64::from(timer.remaining),
            })
            .collect()
    }
}

/// All sources' readings with their slots filled in. A slot the HUD runs ability timers for
/// takes only those: the tray's entry for the same ability would count it twice.
fn merge(by_source: Vec<Vec<Reading>>, abilities: &[String]) -> Vec<Reading> {
    let mut readings: Vec<Reading> = by_source
        .into_iter()
        .flatten()
        .map(|reading| Reading {
            slot: reading.slot.or_else(|| slot(&reading.id, abilities)),
            ..reading
        })
        .collect();
    let timed: HashSet<u8> = readings
        .iter()
        .filter(|r| r.id.starts_with("ability"))
        .filter_map(|r| r.slot)
        .collect();
    readings.retain(|r| r.id.starts_with("ability") || r.slot.is_none_or(|s| !timed.contains(&s)));
    readings
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
        let every = if effects.is_empty() {
            SEARCH_EVERY
        } else {
            RECHECK_EVERY
        };
        if self.searched.is_none_or(|at| at.elapsed() >= every) {
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

fn timers(shown: &[Shown], now_ms: f64, icons: &[(String, String)]) -> Vec<BuffTimer> {
    shown
        .iter()
        .map(|buff| {
            let slot = buff.slot;
            BuffTimer {
                id: buff.id.clone(),
                label: buff.label.clone(),
                slot,
                icon: slot.is_none().then(|| icon(&buff.id, icons)).flatten(),
                total: buff.total,
                ends_at: buff.remaining.map(|left| (now_ms + left * 1000.0).round()),
            }
        })
        .collect()
}

/// The last part of a buff's name: `.../Suits/FairyDustAbilityName#35` gives
/// `FairyDustAbilityName`.
fn key(id: &str) -> &str {
    let tag = id.rsplit_once('#').map_or(id, |(tag, _)| tag);
    tag.rsplit('/').next().unwrap_or(tag)
}

/// The icon whose stem the buff's name starts with, the longest when several do.
fn icon(id: &str, icons: &[(String, String)]) -> Option<String> {
    let key = key(id);
    icons
        .iter()
        .filter(|(stem, _)| !stem.is_empty() && key.starts_with(stem.as_str()))
        .max_by_key(|(stem, _)| stem.len())
        .map(|(_, image)| image.clone())
}

/// The ability slot, counted from 1, whose ability the buff comes from. Buff names share
/// their ability's stem: `.../Suits/FairyDustAbilityName` comes from `.../FairyDustAbility`.
fn slot(id: &str, abilities: &[String]) -> Option<u8> {
    let key = key(id);
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
                && a.icon == b.icon
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
    /// The ability slot, counted from 1, when the source knows it; otherwise found by name.
    slot: Option<u8>,
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
                slot: None,
                remaining: f64::from(e.data),
            })
            .collect()
    }
}

/// Effects the player keeps up by casting: ability buffs and arcanes that trigger on casts.
/// Mod and weapon procs come and go on their own, so they would only crowd the strip.
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
    slot: Option<u8>,
    /// `None` once the buff has dropped off.
    remaining: Option<f64>,
    total: f64,
}

struct Entry {
    shown: Shown,
    gone_at: Option<f64>,
}

/// Turns per-poll readings into what the strip shows. The game only tells us time remaining,
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
                    shown.slot = reading.slot;
                    entry.gone_at = None;
                }
                None => self.entries.push(Entry {
                    shown: Shown {
                        id: reading.id.clone(),
                        label: reading.label.clone(),
                        slot: reading.slot,
                        remaining: Some(reading.remaining),
                        total: reading.remaining,
                    },
                    gone_at: None,
                }),
            }
        }
        for entry in &mut self.entries {
            if entry.shown.remaining.is_some() && !readings.iter().any(|r| r.id == entry.shown.id) {
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
            slot: None,
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
        assert_eq!(
            slot_of("/Lotus/Language/Suits/FairyDustAbilityName"),
            Some(1)
        );
        assert_eq!(
            slot_of("/Lotus/Language/Suits/BrokenRotAbilityName"),
            Some(3)
        );
        assert_eq!(
            slot_of("/Lotus/Language/Suits/FairyFlightAbilityAugment1Name"),
            Some(4)
        );
        assert_eq!(
            slot_of("/Lotus/Language/Arcanes/AbilityDurationOnCastName"),
            None
        );
    }

    #[test]
    fn other_buffs_take_the_icon_their_name_starts_with() {
        let icons = [
            ("AbilityDurationOnCast", "ArcaneProjectionAO.png"),
            (
                "AbilityEfficiencyOnAbilityObjectCreation",
                "WarframeObjectCreation.png",
            ),
            ("PagemasterFinalChapter", "DanteResolution.png"),
            ("Pagemaster", "Wrong.png"),
            ("FairyDust", "FaerieDust.png"),
        ]
        .map(|(stem, image)| (stem.to_owned(), image.to_owned()));
        let icon_of = |tag: &str| icon(&format!("{tag}#7"), &icons);
        assert_eq!(
            icon_of("/Lotus/Language/Arcanes/AbilityDurationOnCastName").as_deref(),
            Some("ArcaneProjectionAO.png")
        );
        assert_eq!(
            icon_of("/Lotus/Language/Suits/PagemasterFinalChapterJourneyName").as_deref(),
            Some("DanteResolution.png")
        );
        assert_eq!(icon_of("/Lotus/Language/Suits/Unknown"), None);

        let titania = ["/Lotus/Powersuits/Fairy/Abilities/FairyDustAbility".to_owned()];
        let shown = |id: &str| Shown {
            id: format!("{id}#1"),
            label: String::new(),
            slot: slot(&format!("{id}#1"), &titania),
            remaining: Some(5.0),
            total: 10.0,
        };
        let sent = timers(
            &[
                shown("/Lotus/Language/Suits/FairyDustAbilityName"),
                shown("/Lotus/Language/Arcanes/AbilityDurationOnCastName"),
            ],
            0.0,
            &icons,
        );
        assert_eq!(
            sent[0].icon, None,
            "slotted buffs sit under their dot instead"
        );
        assert_eq!(sent[1].icon.as_deref(), Some("ArcaneProjectionAO.png"));
    }

    #[test]
    fn ability_timers_take_their_slot_over_the_tray() {
        let titania = [
            "/Lotus/Powersuits/Fairy/Abilities/FairyDustAbility",
            "/Lotus/Powersuits/Fairy/Abilities/FairySoulAbility",
        ]
        .map(String::from);
        let timer = |key: u32, remaining: f64| Reading {
            id: format!("ability1:{key:x}"),
            label: String::new(),
            slot: Some(2),
            remaining,
        };
        let tray = vec![
            r("/Lotus/Language/Suits/FairySoulAbilityName#4", 100.0),
            r("/Lotus/Language/Suits/FairyDustAbilityName#5", 20.0),
        ];
        let merged = merge(vec![tray, vec![timer(1, 118.0), timer(2, 64.0)]], &titania);
        let slots: Vec<_> = merged.iter().map(|r| (r.id.as_str(), r.slot)).collect();
        assert_eq!(
            slots,
            [
                ("/Lotus/Language/Suits/FairyDustAbilityName#5", Some(1)),
                ("ability1:1", Some(2)),
                ("ability1:2", Some(2)),
            ]
        );
    }

    #[test]
    fn countdown_is_not_resent_every_tick() {
        let shown = [Shown {
            id: "a".into(),
            label: "A".into(),
            slot: None,
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
