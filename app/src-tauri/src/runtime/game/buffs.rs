//! Polls the local player's buffs from game memory and feeds the buff strip overlay.
//!
//! Buffs come from more than one place, each a [`BuffSource`]: the HUD's buff tray, and the
//! timers the HUD counts down under the ability icons, which is where abilities like Tribute
//! keep theirs, and the HUDs some abilities draw for themselves.

mod hud_icons;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Runtime};
use tracing::debug;
use wf_mem::MemoryReader;
use wf_scan::{
    AbilityLabel, AbilityTimers, DotsLayout, HudTables, LuaState, LuaTable, StatusEffect,
    ability_dots, ability_hud, ability_huds, hud_hidden, in_gameplay, menu_open, status_effects,
    status_list, switch_index, switched_to,
};

use super::ticks;
use crate::overlay::{self, BuffKind, BuffLayout, BuffTimer, Overlays, SeenBuff, Vitals};
use crate::runtime::attach::{Attached, reattach};
use crate::runtime::blocking;
use crate::settings::BuffSettings;
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
/// it isn't found, as in menus without one. The cap stays short so a mission's HUD is found
/// soon after it loads, however long the player spent outside one.
const HUD_SEARCH_EVERY: Duration = Duration::from_secs(10);
const HUD_SEARCH_MAX: Duration = Duration::from_secs(30);
/// How often the equipped warframe's abilities are looked up again from the inventory.
const LOADOUT_EVERY: Duration = Duration::from_secs(5);
/// An ability HUD whose timers have not moved for this long is left over from an earlier
/// mission, or the game is paused.
const HUD_STALE: Duration = Duration::from_secs(30);

pub(crate) async fn buffs_task<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    let mut tick = ticks(BUFF_TICK);
    let mut game: Option<Attached> = None;
    let mut sources = buff_sources();
    let mut screens = Screens::default();
    let mut tracker = Tracker::new(LINGER);
    let mut sent = Sent::default();
    let mut layout: Option<BuffLayout> = None;
    let started = Instant::now();
    let mut seen: Option<Vec<usize>> = None;
    let mut abilities: Vec<String> = Vec::new();
    let mut icons: Vec<(String, String)> = Vec::new();
    let mut names: Vec<(String, String)> = Vec::new();
    let mut hud_icons: HashMap<hud_icons::Wanted, Option<String>> = HashMap::new();
    let mut loadout_read: Option<Instant> = None;
    // The abilities the HUD names, which follow a warframe switch at once: the inventory's
    // loadout only changes when the game downloads it again, sometimes a mission later.
    let mut live: Option<Vec<String>> = None;
    loop {
        tick.tick().await;
        if loadout_read.is_none_or(|at| at.elapsed() >= LOADOUT_EVERY) {
            loadout_read = Some(Instant::now());
            read_loadout(&state, &mut abilities, &mut icons, &mut names).await;
        }
        let wanted = read(&state.status).pid.is_some()
            && overlay::enabled(&read(&state.settings), overlay::Kind::Buffs);
        let (readings, vitals) = if wanted {
            let owned = Arc::clone(&state);
            let cached = game.take();
            let mut moved = std::mem::take(&mut sources);
            let mut found = std::mem::take(&mut screens);
            let attempt = blocking("read buffs", move || {
                let attached = reattach(&owned, cached)?;
                let (readings, playing) = read_sources(&attached, &mut moved, &mut found);
                let dots = ability_dots(attached.reader.as_ref(), &attached.lua, &mut found.hud);
                let using = moved.iter().find_map(|source| source.abilities());
                // A source keeps the vitals it last read, which go stale once it stops being read.
                let shown = moved
                    .iter()
                    .find_map(|source| source.vitals())
                    .filter(|_| playing);
                anyhow::Ok((attached, moved, found, readings, dots, using, shown))
            })
            .await;
            match attempt {
                Some(Ok((attached, read, found, readings, dots, using, shown))) => {
                    game = Some(attached);
                    if using != live {
                        debug!(?using, "Buff strip slots from the HUD");
                        live = using;
                    }
                    if let Some(dots) = dots {
                        layout = Some(layout_of(dots));
                    }
                    sources = read;
                    screens = found;
                    (Some(readings), shown.map(Vitals::from))
                }
                Some(Err(error)) => {
                    debug!(?error, "Buffs not read");
                    (None, None)
                }
                None => (None, None),
            }
        } else {
            (None, None)
        };

        let now = started.elapsed().as_secs_f64();
        let counts = readings
            .as_ref()
            .map(|by_source| by_source.iter().map(Vec::len).collect::<Vec<_>>());
        let changed = counts != seen;
        if changed {
            seen.clone_from(&counts);
        }
        let idle = readings.is_none();
        let slots = live.as_deref().unwrap_or(&abilities);
        let mut readings = merge(readings.unwrap_or_default(), slots);
        add_hud_icons(&mut readings, &state, slots, &icons, &mut hud_icons).await;
        if changed {
            let read: Vec<_> = readings
                .iter()
                .map(|r| (r.id.as_str(), r.slot, r.remaining))
                .collect();
            debug!(wanted, ?counts, ?read, "Buffs read");
        }
        tracker.update(&readings, now);
        let timers = timers(&tracker.shown(), unix_ms(), &icons, slots, &names);
        let timers = shown_by(
            timers,
            &read(&state.settings).overlays.buffs,
            &state.overlays,
        );
        let vitals = vitals.filter(|_| read(&state.settings).overlays.buffs.buffs_vitals);
        sent.update(&app, &state, timers, layout, vitals);

        if idle {
            game = None;
            sources = buff_sources();
            tokio::time::sleep(IDLE_TICK).await;
        }
    }
}

/// Looks up the equipped warframe's abilities, which give the ability slots, and the game's
/// icons for buffs and names for abilities, keeping what was known when any can't be read.
async fn read_loadout(
    state: &Arc<AppState>,
    abilities: &mut Vec<String>,
    icons: &mut Vec<(String, String)>,
    names: &mut Vec<(String, String)>,
) {
    let owned = Arc::clone(state);
    let looked_up = blocking("read the equipped abilities", move || {
        let core = lock(&owned.core);
        (
            core.equipped_abilities(),
            core.buff_icons(),
            core.ability_names(),
        )
    })
    .await;
    let (equipped, known, titles) = looked_up.unwrap_or_default();
    if !titles.is_empty() {
        *names = titles;
    }
    let equipped = equipped.unwrap_or_default();
    if !known.is_empty() {
        *icons = known;
    }
    if equipped != *abilities {
        debug!(?equipped, "Buff strip slots");
        *abilities = equipped;
    }
}

/// Gives buffs an ability's own HUD names the game's icons for them, looking up in the game's
/// cache the ones not looked up before.
async fn add_hud_icons(
    readings: &mut [Reading],
    state: &AppState,
    abilities: &[String],
    icons: &[(String, String)],
    known: &mut HashMap<hud_icons::Wanted, Option<String>>,
) {
    let missing: Vec<hud_icons::Wanted> = readings
        .iter()
        .filter_map(|reading| hud_icon_key(reading, abilities, icons))
        .filter(|key| !known.contains_key(key))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    if !missing.is_empty() {
        let log = read(&state.settings).log_path();
        let images = state.images.dir().to_owned();
        let found = blocking("find ability HUD icons", move || {
            hud_icons::find(log.as_deref(), &images, missing)
        })
        .await;
        known.extend(found.unwrap_or_default());
    }
    for reading in readings {
        reading.icon = hud_icon_key(reading, abilities, icons)
            .and_then(|key| known.get(&key).cloned().flatten());
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

    /// The abilities the player is using, in slot order, when the source can tell.
    fn abilities(&self) -> Option<Vec<String>> {
        None
    }

    /// The player's health, shields and energy as the HUD shows them, when the source can tell.
    fn vitals(&self) -> Option<wf_scan::Vitals> {
        None
    }
}

fn buff_sources() -> Vec<Box<dyn BuffSource>> {
    vec![
        Box::new(TraySource::default()),
        Box::new(AbilityTimerSource::default()),
        Box::new(AbilityHudSource::default()),
    ]
}

/// HUDs ability scripts draw themselves, like Tribute's quiver of buffs above the weapon.
/// Each names its ability, so its entries take that ability's slot. Every entry is read, ones
/// not running too, so the slot shows all of the ability's buffs.
#[derive(Default)]
struct AbilityHudSource {
    /// Each HUD with the buffs its script describes, by their place in its list.
    huds: Vec<(LuaTable, BTreeMap<usize, String>)>,
    /// Where each HUD's script keeps which entry the player has switched to.
    switches: HashMap<LuaTable, String>,
    searched: Option<Instant>,
    /// Each HUD's timers when last read, and when one of them last fell. A HUD left over from
    /// an earlier mission keeps its last values, which never fall.
    last: HashMap<LuaTable, (Vec<f32>, Option<Instant>)>,
    /// The most seconds each HUD has shown on an entry: what the game sets an entry to when it
    /// is cast, so its buffs' full length.
    longest: HashMap<LuaTable, f32>,
}

impl BuffSource for AbilityHudSource {
    fn read(&mut self, reader: &Arc<dyn MemoryReader>, lua: &LuaState) -> Vec<Reading> {
        let reader = reader.as_ref();
        if self.searched.is_none_or(|at| at.elapsed() >= RECHECK_EVERY) {
            self.searched = Some(Instant::now());
            self.huds = ability_huds(reader, lua);
            self.switches = self
                .huds
                .iter()
                .filter_map(|&(table, _)| Some((table, switch_index(reader, lua, table)?)))
                .collect();
            let kept = |table: &LuaTable| self.huds.iter().any(|(hud, _)| hud == table);
            self.last.retain(|table, _| kept(table));
            self.longest.retain(|table, _| kept(table));
        }
        let mut readings = Vec::new();
        for (hud, (table, names)) in self.huds.iter().enumerate() {
            let table = *table;
            let Some(shown) = ability_hud(reader, table) else {
                continue;
            };
            let (last, moved_at) = self.last.entry(table).or_default();
            if last.len() == shown.timers.len()
                && last.iter().zip(&shown.timers).any(|(was, now)| now < was)
            {
                *moved_at = Some(Instant::now());
            }
            last.clone_from(&shown.timers);
            let all_down = shown.timers.iter().all(|&seconds| seconds <= 0.0);
            let live = moved_at.is_some_and(|at| all_down || at.elapsed() < HUD_STALE);
            let longest = self.longest.entry(table).or_default();
            *longest = shown.timers.iter().copied().fold(*longest, f32::max);
            if !live {
                continue;
            }
            let count = shown.timers.len();
            let selected = self
                .switches
                .get(&table)
                .and_then(|name| switched_to(reader, lua, name, count));
            readings.extend(shown.timers.iter().enumerate().map(|(index, &seconds)| {
                Reading {
                    id: format!("ability{}:hud{hud}.{index}", shown.ability),
                    label: names
                        .get(&index)
                        .map_or_else(String::new, |name| title(name)),
                    slot: u8::try_from(shown.ability + 1).ok(),
                    rank: selected
                        .and_then(|selected| u8::try_from((index + count - selected) % count).ok()),
                    buff: names.get(&index).cloned(),
                    icon: None,
                    total: Some(f64::from(*longest)),
                    remaining: f64::from(seconds.max(0.0)),
                }
            }));
        }
        readings
    }
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

/// The timers the HUD runs under each ability icon, one per cast still running, like each of
/// Protea's grenades. These carry no names, only which ability they belong to, so they take
/// that ability's slot, as one buff following the cast that runs out first. Cooldowns the
/// game keeps on the abilities themselves, like Omamori's, take their ability's slot too.
#[derive(Default)]
struct AbilityTimerSource {
    timers: Option<AbilityTimers>,
    searched: Option<Instant>,
    search: Option<mpsc::Receiver<Option<AbilityTimers>>>,
    /// Searches in a row that found nothing.
    misses: u32,
    falling: Falling,
    /// The suit's abilities as the HUD last named them.
    abilities: Option<Vec<String>>,
    /// What the HUD last showed of the player's health, shields and energy.
    vitals: Option<wf_scan::Vitals>,
}

/// Keeps only readings whose value has been seen falling. The HUD lists counters under
/// ability icons the same way as timers, like an augment's stacks, and only timers count down.
#[derive(Default)]
struct Falling {
    last: HashMap<String, f64>,
    seen: HashSet<String>,
}

impl Falling {
    fn keep(&mut self, readings: Vec<Reading>) -> Vec<Reading> {
        for reading in &readings {
            if self
                .last
                .get(&reading.id)
                .is_some_and(|&prev| reading.remaining < prev)
            {
                self.seen.insert(reading.id.clone());
            }
        }
        self.last = readings
            .iter()
            .map(|r| (r.id.clone(), r.remaining))
            .collect();
        self.seen.retain(|id| self.last.contains_key(id));
        readings
            .into_iter()
            .filter(|r| self.seen.contains(&r.id))
            .collect()
    }
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
        let cooldowns = self
            .timers
            .and_then(|timers| timers.cooldowns(reader.as_ref()))
            .unwrap_or_default();
        self.abilities = self
            .timers
            .and_then(|timers| timers.abilities(reader.as_ref()));
        self.vitals = self
            .timers
            .and_then(|timers| timers.vitals(reader.as_ref()));
        if read.is_none() {
            self.search(reader);
        }
        let readings = read
            .unwrap_or_default()
            .into_iter()
            .map(|timer| Reading {
                id: format!("ability{}:{:x}", timer.ability, timer.key),
                label: String::new(),
                slot: u8::try_from(timer.ability + 1).ok(),
                rank: None,
                buff: None,
                icon: None,
                total: None,
                remaining: f64::from(timer.remaining),
            })
            .collect();
        let mut readings = first_ending(self.falling.keep(readings));
        let labels = self
            .timers
            .and_then(|timers| timers.labels(reader.as_ref()))
            .unwrap_or_default();
        readings.extend(from_labels(&readings, labels));
        // A cooldown always counts down, and is the ability's own, apart from its casts'.
        readings.extend(cooldowns.into_iter().map(|cooldown| Reading {
            id: format!("ability{}:cooldown", cooldown.ability),
            label: String::new(),
            slot: u8::try_from(cooldown.ability + 1).ok(),
            rank: None,
            buff: None,
            icon: None,
            total: None,
            remaining: f64::from(cooldown.remaining),
        }));
        readings
    }

    fn abilities(&self) -> Option<Vec<String>> {
        self.abilities.clone()
    }

    fn vitals(&self) -> Option<wf_scan::Vitals> {
        self.vitals
    }
}

/// Readings for the seconds ability scripts write under their icons themselves, like Grasp of
/// Lohk's, for abilities the HUD runs no timer for. The HUD only shows them while they run,
/// and they hold still while the ability is paused, as under The Vast Untime, so unlike its
/// timers they needn't be seen falling.
fn from_labels(timed: &[Reading], labels: Vec<AbilityLabel>) -> Vec<Reading> {
    labels
        .into_iter()
        .filter(|label| {
            let ability = format!("ability{}", label.ability);
            !timed.iter().any(|reading| reading.id == ability)
        })
        .map(|label| Reading {
            id: format!("ability{}:label", label.ability),
            label: String::new(),
            slot: u8::try_from(label.ability + 1).ok(),
            rank: None,
            buff: None,
            icon: None,
            total: None,
            remaining: f64::from(label.remaining),
        })
        .collect()
}

/// One reading per ability: the cast that runs out first, under an id of the ability's alone so
/// it stays one buff as casts run out and new ones start.
fn first_ending(readings: Vec<Reading>) -> Vec<Reading> {
    let mut by_ability: BTreeMap<String, Reading> = BTreeMap::new();
    for reading in readings {
        let ability = reading.id.split(':').next().unwrap_or_default().to_owned();
        let sooner = by_ability
            .get(&ability)
            .is_none_or(|kept| reading.remaining < kept.remaining);
        if sooner {
            by_ability.insert(
                ability.clone(),
                Reading {
                    id: ability,
                    ..reading
                },
            );
        }
    }
    by_ability.into_values().collect()
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
            .map_or_default(|list| status_effects(reader, list));
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
                .map_or_default(|list| status_effects(reader, list));
        }
        effects
    }
}

/// What the strip shows of each buff. Every one says what it is, so none shows as a bare
/// number away from its ability's dot: a buff named in game keeps its name, one in an ability
/// slot takes its ability's, and one past the dots takes its ability's icon where the game has
/// one, like an operator ability's cooldown.
fn timers(
    shown: &[Shown],
    now_ms: f64,
    icons: &[(String, String)],
    abilities: &[String],
    names: &[(String, String)],
) -> Vec<BuffTimer> {
    shown
        .iter()
        .map(|buff| {
            let slot = buff.slot;
            let ability = slot
                .and_then(|slot| slot.checked_sub(1))
                .and_then(|index| abilities.get(usize::from(index)));
            let label = if !buff.label.is_empty() {
                buff.label.clone()
            } else if let Some(ability) = ability {
                ability_name(ability, names)
            } else {
                title(key(&buff.id))
            };
            let past_dots = slot.is_some_and(|slot| slot > DOTS);
            BuffTimer {
                id: buff.id.clone(),
                label,
                slot,
                rank: buff.rank,
                icon: buff.icon.clone().or_else(|| match (slot, ability) {
                    (None, _) => icon(&buff.id, icons),
                    (Some(_), Some(ability)) if past_dots => icon(ability, icons),
                    (Some(_), _) => None,
                }),
                total: buff.total,
                ends_at: buff.remaining.map(|left| (now_ms + left * 1000.0).round()),
            }
        })
        .collect()
}

/// The ability dots the strip draws round; slots past them, like the operator's abilities,
/// sit beside them instead.
const DOTS: u8 = 4;

/// An ability's in-game name, `Grasp of Lohk` for `.../BrokenGraspAbility`. The operator's
/// aren't any warframe's, so they have their own; anything else is read from its own name.
fn ability_name(ability: &str, names: &[(String, String)]) -> String {
    let key = key(ability);
    match key {
        "OperatorFocusPowerAbility" => return "Focus".into(),
        "OperatorTransferenceAbility" => return "Transference".into(),
        _ => {}
    }
    names
        .iter()
        .find(|(stem, _)| stem == key)
        .map_or_else(|| title(key), |(_, name)| name.clone())
}

/// A readable name from a game identifier: `thorns` gives `Thorns`, `FairyDustAbilityName`
/// gives `Fairy Dust`.
fn title(name: &str) -> String {
    let words = label(name);
    let mut chars = words.chars();
    chars.next().map_or_else(
        || "Buff".into(),
        |first| first.to_uppercase().chain(chars).collect(),
    )
}

/// A buff's id without the instance number the game gives each application of it:
/// `.../Suits/ShedAbilityName#37` gives `.../Suits/ShedAbilityName`.
fn instance_of(id: &str) -> &str {
    id.rsplit_once('#').map_or(id, |(tag, _)| tag)
}

/// Where the scripts saying which screen is up were last found.
#[derive(Default)]
struct Screens {
    chat: Option<LuaTable>,
    squad: Option<LuaTable>,
    /// The HUD, for where it draws the ability dots.
    hud: HudTables,
}

fn layout_of(dots: DotsLayout) -> BuffLayout {
    BuffLayout {
        stage_width: dots.stage_width,
        stage_height: dots.stage_height,
        x: dots.x,
        y: dots.y,
        hud_scale: dots.hud_scale,
    }
}

/// Reads every source, or none out of a mission, with the Esc menu up or while the game hides
/// its HUD: once a mission ends, the HUD's last timers linger in memory, frozen, and the menu
/// and the end-of-mission screen cover the HUD. Also says whether the sources were read.
fn read_sources(
    attached: &Attached,
    sources: &mut [Box<dyn BuffSource>],
    screens: &mut Screens,
) -> (Vec<Vec<Reading>>, bool) {
    let reader = attached.reader.as_ref();
    let playing = in_gameplay(reader, &attached.lua, &mut screens.chat) != Some(false)
        && menu_open(reader, &attached.lua, &mut screens.squad) != Some(true)
        && hud_hidden(reader, &attached.lua) != Some(true);
    let readings = sources
        .iter_mut()
        .map(|source| {
            if playing {
                source.read(&attached.reader, &attached.lua)
            } else {
                Vec::new()
            }
        })
        .collect();
    (readings, playing)
}

/// What to look up the game's icon for a buff an ability's own HUD names by: the ability's
/// icon, and the buff's name.
fn hud_icon_key(
    reading: &Reading,
    abilities: &[String],
    icons: &[(String, String)],
) -> Option<hud_icons::Wanted> {
    let buff = reading.buff.clone()?;
    let ability = abilities.get(usize::from(reading.slot?.checked_sub(1)?))?;
    Some((icon(ability, icons)?, buff))
}

/// Where a buff comes from: an equipped ability's slot, else a warframe's ability, which is a
/// squadmate's, else the player's own arcanes, mods and the like.
fn kind_of(timer: &BuffTimer) -> BuffKind {
    if timer.slot.is_some() {
        BuffKind::Ability
    } else if timer.id.contains("/Suits/") {
        BuffKind::Squad
    } else {
        BuffKind::Own
    }
}

/// The buffs the settings show. Every buff outside the ability slots is noted as seen, shown or
/// not, so the settings can offer hiding or showing it by name.
fn shown_by(
    timers: Vec<BuffTimer>,
    settings: &BuffSettings,
    overlays: &Overlays,
) -> Vec<BuffTimer> {
    overlays.saw_buffs(
        timers
            .iter()
            .filter(|timer| timer.slot.is_none())
            .map(|timer| SeenBuff {
                name: key(&timer.id).to_owned(),
                kind: kind_of(timer),
                icon: timer.icon.clone(),
            }),
    );
    timers
        .into_iter()
        .filter(|timer| {
            let wanted = match kind_of(timer) {
                BuffKind::Ability => settings.buffs_abilities,
                BuffKind::Own => settings.buffs_own,
                BuffKind::Squad => settings.buffs_squad,
            };
            wanted
                && !settings
                    .buffs_hidden
                    .iter()
                    .any(|name| name == key(&timer.id))
        })
        .collect()
}

/// The last part of a buff's name: `.../Suits/FairyDustAbilityName#35` gives
/// `FairyDustAbilityName`.
fn key(id: &str) -> &str {
    let tag = instance_of(id);
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

/// What the strip was last sent, so it is only sent again when that changes.
#[derive(Default)]
struct Sent {
    timers: Vec<BuffTimer>,
    layout: Option<BuffLayout>,
    vitals: Option<Vitals>,
}

impl Sent {
    fn update<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        state: &Arc<AppState>,
        timers: Vec<BuffTimer>,
        layout: Option<BuffLayout>,
        vitals: Option<Vitals>,
    ) {
        let strip = same(&timers, &self.timers) && layout == self.layout;
        if strip && vitals == self.vitals {
            return;
        }
        // Energy and health change many times a second; while the strip stays up, only its
        // numbers need passing on.
        if strip && vitals.is_some() && self.vitals.is_some() {
            overlay::on_vitals(app, state, vitals);
        } else {
            overlay::on_buffs(app, state, timers.clone(), layout, vitals);
        }
        *self = Self {
            timers,
            layout,
            vitals,
        };
    }
}

fn same(now: &[BuffTimer], sent: &[BuffTimer]) -> bool {
    now.len() == sent.len()
        && now.iter().zip(sent).all(|(a, b)| {
            a.id == b.id
                && a.label == b.label
                && a.slot == b.slot
                && a.rank == b.rank
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
    /// Its place in its ability's own HUD counting on from the one the player has switched
    /// to, which is 0: the order the HUD cycles through them.
    rank: Option<u8>,
    /// The name the ability's script gives it.
    buff: Option<String>,
    /// The game's icon for it, by image name, when it has its own.
    icon: Option<String>,
    /// The buff's full length when the game says; otherwise learned from what is seen.
    total: Option<f64>,
    remaining: f64,
}

/// Which effects count down: the game says an effect's value is seconds, and it has been seen
/// falling, since the tray keeps frozen entries around. Counts fall too, like Grasp of Lohk's
/// weapons as each expires, so falling alone doesn't make a timer.
#[derive(Default)]
struct Ticking {
    last: HashMap<u32, f32>,
    timed: HashSet<u32>,
}

impl Ticking {
    fn timed(&mut self, effects: &[StatusEffect]) -> Vec<Reading> {
        for effect in effects {
            let fell = self
                .last
                .get(&effect.id)
                .is_some_and(|&prev| effect.data < prev);
            if fell && self.timed.insert(effect.id) && !effect.is_timed() {
                debug!(
                    tag = %effect.name_tag,
                    buff_type = effect.buff_type,
                    data = effect.data,
                    "Buff falls but isn't a timer"
                );
            }
        }
        self.last = effects.iter().map(|e| (e.id, e.data)).collect();
        self.timed.retain(|id| self.last.contains_key(id));
        effects
            .iter()
            .filter(|e| {
                e.is_timed() && !e.is_debuff && cast(&e.name_tag) && self.timed.contains(&e.id)
            })
            .map(|e| Reading {
                id: format!("{}#{}", e.name_tag, e.id),
                label: label(&e.name_tag),
                slot: None,
                rank: None,
                buff: None,
                icon: None,
                total: (e.initial_data >= e.data).then_some(f64::from(e.initial_data)),
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
    rank: Option<u8>,
    icon: Option<String>,
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
            if let Some(entry) = self.entries.iter_mut().find(|e| e.shown.id == reading.id) {
                let shown = &mut entry.shown;
                let refreshed = shown
                    .remaining
                    .is_none_or(|old| reading.remaining > old + REFRESH_JUMP);
                shown.total = reading.total.unwrap_or(if refreshed {
                    reading.remaining
                } else {
                    shown.total.max(reading.remaining)
                });
                shown.remaining = Some(reading.remaining);
                shown.label.clone_from(&reading.label);
                shown.slot = reading.slot;
                shown.rank = reading.rank;
                shown.icon.clone_from(&reading.icon);
                entry.gone_at = None;
            } else {
                let entry = Entry {
                    shown: Shown {
                        id: reading.id.clone(),
                        label: reading.label.clone(),
                        slot: reading.slot,
                        rank: reading.rank,
                        icon: reading.icon.clone(),
                        remaining: Some(reading.remaining),
                        total: reading.total.unwrap_or(reading.remaining),
                    },
                    gone_at: None,
                };
                // A recast comes back under a new instance number; it takes the place of
                // the one it replaces rather than showing beside it.
                let replaced = self.entries.iter().position(|e| {
                    instance_of(&e.shown.id) == instance_of(&reading.id)
                        && !readings.iter().any(|r| r.id == e.shown.id)
                });
                match replaced {
                    Some(index) => self.entries[index] = entry,
                    None => self.entries.push(entry),
                }
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

    #[test]
    fn ability_casts_show_as_the_first_ending() {
        let read = first_ending(vec![
            r("ability0:7", 18.2),
            r("ability0:8", 12.5),
            r("ability0:9", 16.0),
            r("ability2:1", 4.0),
        ]);
        let read: Vec<_> = read.iter().map(|r| (r.id.as_str(), r.remaining)).collect();
        assert_eq!(read, [("ability0", 12.5), ("ability2", 4.0)]);
    }

    #[test]
    fn every_buff_says_what_it_is() {
        let xaku = [
            "BrokenRotAbility",
            "BrokenGraspAbility",
            "BrokenEmbraceAbility",
            "BrokenDestructAbility",
            "OperatorFocusPowerAbility",
        ]
        .map(String::from);
        let names = [("BrokenGraspAbility".to_owned(), "Grasp of Lohk".to_owned())];
        let icons = [("OperatorFocusPower".to_owned(), "Focus.png".to_owned())];
        let shown = |id: &str, label: &str, slot: Option<u8>| Shown {
            id: id.into(),
            label: label.into(),
            slot,
            rank: None,
            icon: None,
            remaining: Some(5.0),
            total: 10.0,
        };
        let sent = timers(
            &[
                shown("ability1:label", "", Some(2)),
                shown("ability4:cooldown", "", Some(5)),
                shown("ability0:hud0.1", "Thorns", Some(1)),
                shown("/Lotus/Language/Suits/OddTag#3", "", None),
            ],
            0.0,
            &icons,
            &xaku,
            &names,
        );
        let said: Vec<_> = sent
            .iter()
            .map(|t| (t.label.as_str(), t.icon.as_deref()))
            .collect();
        assert_eq!(
            said,
            [
                ("Grasp of Lohk", None),
                ("Focus", Some("Focus.png")),
                ("Thorns", None),
                ("Odd Tag", None),
            ]
        );
    }

    #[test]
    fn labels_fill_abilities_without_timers() {
        let timed = [Reading {
            slot: Some(4),
            ..r("ability3", 12.0)
        }];
        let labels = vec![
            AbilityLabel {
                ability: 1,
                remaining: 17.0,
            },
            AbilityLabel {
                ability: 3,
                remaining: 12.0,
            },
        ];
        let read = from_labels(&timed, labels);
        let read: Vec<_> = read
            .iter()
            .map(|r| (r.id.as_str(), r.slot, r.remaining))
            .collect();
        assert_eq!(read, [("ability1:label", Some(2), 17.0)]);
    }

    #[test]
    fn ability_counters_are_not_timers() {
        let mut falling = Falling::default();
        let read = |remaining: [f64; 2]| {
            vec![
                Reading {
                    slot: Some(2),
                    ..r("ability1:a", remaining[0])
                },
                Reading {
                    slot: Some(4),
                    ..r("ability3:0", remaining[1])
                },
            ]
        };
        assert!(falling.keep(read([120.0, 6.0])).is_empty());
        let kept = falling.keep(read([119.9, 7.0]));
        assert_eq!(
            kept.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["ability1:a"]
        );
        assert_eq!(falling.keep(read([119.8, 7.0])).len(), 1);
    }

    fn r(id: &str, remaining: f64) -> Reading {
        Reading {
            id: id.into(),
            label: id.into(),
            slot: None,
            rank: None,
            buff: None,
            icon: None,
            total: None,
            remaining,
        }
    }

    fn effect(id: u32, data: f32) -> StatusEffect {
        StatusEffect {
            id,
            name_tag: format!("/Lotus/Language/Suits/E{id}AbilityName"),
            buff_type: 1,
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
    fn recast_replaces_its_old_instance() {
        let mut t = Tracker::new(3.0);
        t.update(&[r("x#1", 5.0), r("shed#31", 2.0)], 0.0);
        t.update(&[r("x#1", 4.0), r("shed#34", 31.0)], 1.0);
        let ids: Vec<_> = t.shown().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, ["x#1", "shed#34"]);
        t.update(
            &[r("x#1", 3.0), r("shed#34", 30.0), r("shed#35", 31.0)],
            2.0,
        );
        assert_eq!(t.shown().len(), 3);
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
    fn falling_counts_are_not_timers() {
        // Grasp of Lohk's tray entry counts the weapons held, which falls as each expires.
        let mut t = Ticking::default();
        let mut grasp = effect(8, 10.0);
        grasp.name_tag = "/Lotus/Language/Suits/BrokenGraspAbilityName".into();
        grasp.buff_type = 5;
        t.timed(&[grasp.clone()]);
        grasp.data = 9.0;
        assert!(t.timed(&[grasp]).is_empty());
    }

    #[test]
    fn timed_stat_bonuses_are_timers() {
        // Shock's augment reads as type 3, not the abilities' 1.
        let mut t = Ticking::default();
        let mut shock = effect(9, 63.2);
        shock.name_tag = "/Lotus/Language/Suits/ShockAbilityAugment1Name".into();
        shock.buff_type = 3;
        t.timed(&[shock.clone()]);
        shock.data = 62.2;
        assert_eq!(t.timed(&[shock]).len(), 1);
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
            rank: None,
            icon: None,
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
            &titania,
            &[],
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
            rank: None,
            buff: None,
            icon: None,
            total: None,
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
            rank: None,
            icon: None,
            remaining: Some(10.0),
            total: 20.0,
        }];
        let first = timers(&shown, 0.0, &[], &[], &[]);
        let later = timers(
            &[Shown {
                remaining: Some(9.9),
                ..shown[0].clone()
            }],
            100.0,
            &[],
            &[],
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
            &[],
            &[],
        );
        assert!(!same(&recast, &first));
    }
}
