use std::collections::{BTreeSet, HashSet};

use chrono::{DateTime, TimeDelta, Utc};
use wf_log::{Event as LogEvent, is_arbitration_drone};
use wf_worldstate::{node_and_planet, node_name};

use super::boosters::ArbitrationRunBoosterTracker;
use super::{
    ArbitrationRun, ArbitrationRunReward, ArbitrationRunRotation, ArbitrationRunSample,
    ArbitrationRunSource, ArbitrationRunStatus, ArbitrationRunWave,
};

const SAMPLE_BUCKET_SECS: f64 = 5.0;
const OUTCOME_CORROBORATION_SECS: f64 = 10.0;
const ROTATION_REPEAT_SECS: f64 = 30.0;
const WAVES_PER_ROTATION: usize = 3;
const MIRROR_DEFENSE_WAVES_PER_ROTATION: usize = 2;
const MIRROR_DEFENSE_NODES: [&str; 2] = ["SolNode719", "SolNode450"];

pub(super) fn log_offset(secs: f64) -> Option<TimeDelta> {
    let offset = std::time::Duration::try_from_secs_f64(secs).ok()?;
    TimeDelta::from_std(offset).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum CaptureNote {
    NoStart,
    NoClock,
    LogEnd,
    NextMission,
    ClockRestart,
    CounterReset,
    SpawnCounting,
    ClientSide,
}

impl CaptureNote {
    pub(super) fn text(self) -> &'static str {
        match self {
            Self::NoStart => {
                "The mission start is missing, elapsed time begins at the first arbitration line."
            }
            Self::NoClock => "The log has no clock line, so the run has no calendar time.",
            Self::LogEnd => "The log ends before the mission did.",
            Self::NextMission => "The next mission began before this one ended.",
            Self::ClockRestart => "The game restarted during the run.",
            Self::CounterReset => {
                "The game's spawn counter reset, the total only counts logged increments."
            }
            Self::SpawnCounting => {
                "These are not kills. Enemy spawns is the game's spawn counter, drones are logged drone spawns."
            }
            Self::ClientSide => {
                "Recorded as a client, spawns and drones are only available for the host."
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ArbitrationMissionStart {
    Unobserved,
    Observed { secs: f64 },
}

#[derive(Clone, Copy)]
enum ArbitrationMissionEnd {
    Open,
    Ended {
        secs: f64,
        status: ArbitrationRunStatus,
        certain: bool,
    },
    Interrupted,
}

impl ArbitrationMissionEnd {
    fn status(self) -> ArbitrationRunStatus {
        match self {
            Self::Open => ArbitrationRunStatus::Recording,
            Self::Ended { status, .. } => status,
            Self::Interrupted => ArbitrationRunStatus::Interrupted,
        }
    }

    fn record(&mut self, secs: f64, status: ArbitrationRunStatus, certain: bool) {
        let fallback = status == ArbitrationRunStatus::Ended;
        let conclusive = certain || !fallback;
        *self = match *self {
            Self::Ended {
                secs: before,
                status: known,
                certain: true,
            } if fallback => Self::Ended {
                secs: before.max(secs),
                status: known,
                certain: true,
            },
            Self::Ended {
                secs: before,
                certain: certain_before,
                ..
            } => Self::Ended {
                secs: before.max(secs),
                status,
                certain: certain_before || conclusive,
            },
            Self::Open | Self::Interrupted => Self::Ended {
                secs,
                status,
                certain: conclusive,
            },
        };
    }

    fn interrupt(&mut self) {
        if let Self::Open = *self {
            *self = Self::Interrupted;
        }
    }
}

#[derive(Clone, Copy)]
struct OutcomeHint {
    status: ArbitrationRunStatus,
    at_secs: f64,
}

#[derive(Clone, Copy)]
enum SpawnCounter {
    Unseen,
    Seen { value: u32, at_secs: f64 },
}

pub(super) struct ActiveRun {
    node_id: Option<String>,
    mission_type: Option<String>,
    host: Option<bool>,
    first_secs: f64,
    last_secs: f64,
    last_activity_secs: f64,
    start: ArbitrationMissionStart,
    end: ArbitrationMissionEnd,
    outcome_hint: Option<OutcomeHint>,
    gameplay_start_secs: Option<f64>,
    spawn_counter: SpawnCounter,
    enemy_spawns: u32,
    peak_live_agents: Option<u32>,
    drone_agents: HashSet<String>,
    drone_spawns: u32,
    rotations: Vec<ArbitrationRunRotation>,
    waves: Vec<ArbitrationRunWave>,
    samples: Vec<ArbitrationRunSample>,
    notes: BTreeSet<CaptureNote>,
    origin: Option<DateTime<Utc>>,
    rewards: Option<Vec<ArbitrationRunReward>>,
    collected_vitus: Option<u32>,
}

impl ActiveRun {
    pub(super) fn new(
        secs: f64,
        node_id: Option<String>,
        mission_type: Option<String>,
        origin: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            node_id,
            mission_type,
            host: None,
            first_secs: secs,
            last_secs: secs,
            last_activity_secs: secs,
            start: ArbitrationMissionStart::Unobserved,
            end: ArbitrationMissionEnd::Open,
            outcome_hint: None,
            gameplay_start_secs: None,
            spawn_counter: SpawnCounter::Unseen,
            enemy_spawns: 0,
            peak_live_agents: None,
            drone_agents: HashSet::new(),
            drone_spawns: 0,
            rotations: Vec::new(),
            waves: Vec::new(),
            samples: Vec::new(),
            notes: BTreeSet::new(),
            origin,
            rewards: None,
            collected_vitus: None,
        }
    }

    pub(super) fn attach_mission_end(
        &mut self,
        rewards: &[ArbitrationRunReward],
        collected_vitus: Option<u32>,
    ) -> bool {
        if !self.end_observed() || self.rewards.is_some() {
            return false;
        }

        self.rewards = Some(rewards.to_vec());
        self.collected_vitus = collected_vitus;
        true
    }

    pub(super) fn node_id(&self) -> Option<&str> {
        self.node_id.as_deref()
    }

    pub(super) fn started(&self) -> bool {
        matches!(self.start, ArbitrationMissionStart::Observed { .. })
    }

    pub(super) fn end_observed(&self) -> bool {
        matches!(self.end, ArbitrationMissionEnd::Ended { .. })
    }

    fn start_secs(&self) -> f64 {
        match self.start {
            ArbitrationMissionStart::Unobserved => self.first_secs,
            ArbitrationMissionStart::Observed { secs } => secs,
        }
    }

    fn note_activity(&mut self, secs: f64) {
        self.last_activity_secs = self.last_activity_secs.max(secs);
    }

    fn capture_broken(&self) -> bool {
        self.notes.contains(&CaptureNote::CounterReset)
    }

    pub(super) fn set_node(&mut self, node_id: &str) {
        if !self.started() || self.node_id.is_none() {
            self.node_id = Some(node_id.to_owned());
        }
    }

    pub(super) fn set_mission_type(&mut self, mission_type: String) {
        self.mission_type = Some(mission_type);
    }

    pub(super) fn set_host(&mut self, host: bool) {
        self.host = Some(host);
    }

    fn host_telemetry(&self) -> bool {
        matches!(self.spawn_counter, SpawnCounter::Seen { .. }) || self.drone_spawns > 0
    }

    fn host(&self) -> bool {
        self.host.unwrap_or_else(|| self.host_telemetry())
    }

    pub(super) fn mission_started(&mut self, secs: f64) {
        if !self.started() {
            self.start = ArbitrationMissionStart::Observed { secs };
        }
    }

    pub(super) fn interrupt(&mut self, note: CaptureNote) {
        if !self.end_observed() {
            self.end.interrupt();
            self.notes.insert(note);
        }
    }

    pub(super) fn advance(&mut self, secs: f64) {
        self.last_secs = self.last_secs.max(secs);
    }

    fn mirror_defense(&self) -> bool {
        self.mission_type.as_deref() == Some("Mirror Defense")
            || self
                .node_id
                .as_deref()
                .is_some_and(|id| MIRROR_DEFENSE_NODES.contains(&id))
    }

    fn corroborated_outcome(&self, secs: f64) -> ArbitrationRunStatus {
        if self.end.status() == ArbitrationRunStatus::Aborted {
            return ArbitrationRunStatus::Aborted;
        }

        match self.outcome_hint {
            Some(hint)
                if secs - hint.at_secs <= OUTCOME_CORROBORATION_SECS
                    && hint.at_secs >= self.last_activity_secs =>
            {
                hint.status
            }
            _ => ArbitrationRunStatus::Ended,
        }
    }

    fn rotation(&mut self, secs: f64, started_secs: Option<f64>) {
        let repeated = self
            .rotations
            .iter()
            .any(|rotation| (secs - rotation.at_secs).abs() <= ROTATION_REPEAT_SECS);
        if repeated {
            return;
        }

        self.rotations.push(ArbitrationRunRotation {
            index: self
                .rotations
                .last()
                .map_or(1, |previous| previous.index + 1),
            at_secs: secs,
            duration_secs: started_secs
                .filter(|started| *started >= self.first_secs - 1.0 && secs > *started)
                .map(|started| secs - started),
        });
        self.note_activity(secs);
    }

    fn survival_reward(&mut self, secs: f64, elapsed_secs: f64) {
        if elapsed_secs <= 0.0 {
            return;
        }

        let started = self
            .rotations
            .last()
            .map_or(secs - elapsed_secs, |previous| previous.at_secs);
        self.rotation(secs, Some(started.max(self.start_secs())));
    }

    fn rotation_rewarded(&mut self, secs: f64) {
        let started = self
            .rotations
            .last()
            .map(|previous| previous.at_secs)
            .or(self.gameplay_start_secs);
        self.rotation(secs, started);
    }

    fn wave(&mut self, index: u32, secs: f64) {
        if index == 0 || self.waves.iter().any(|wave| wave.index == index) {
            return;
        }

        if let Some(last) = self.waves.last_mut()
            && index == last.index + 1
        {
            last.complete = true;
        }

        if index == 1 {
            self.gameplay_start_secs.get_or_insert(secs);
        }

        self.waves.push(ArbitrationRunWave {
            index,
            complete: false,
        });
        self.note_activity(secs);
    }

    fn wave_cleared(&mut self) {
        if let Some(last) = self.waves.last_mut() {
            last.complete = true;
        }
    }

    fn count_spawns(&mut self, spawned: u32, secs: f64) {
        self.spawn_counter = match self.spawn_counter {
            SpawnCounter::Unseen => SpawnCounter::Seen {
                value: spawned,
                at_secs: self.first_secs.max(secs),
            },
            SpawnCounter::Seen { value, at_secs } if spawned >= value => {
                self.enemy_spawns += spawned - value;
                SpawnCounter::Seen {
                    value: spawned,
                    at_secs: at_secs.max(secs),
                }
            }
            SpawnCounter::Seen { at_secs, .. } if secs >= at_secs => {
                self.notes.insert(CaptureNote::CounterReset);
                SpawnCounter::Seen {
                    value: spawned,
                    at_secs: secs,
                }
            }
            seen @ SpawnCounter::Seen { .. } => seen,
        };
    }

    fn agent(&mut self, secs: f64, agent: &str, live: u32, spawned: u32) {
        if is_arbitration_drone(agent) && self.drone_agents.insert(agent.to_owned()) {
            self.drone_spawns += 1;
            self.note_activity(secs);
        }

        self.count_spawns(spawned, secs);
        self.peak_live_agents = Some(self.peak_live_agents.map_or(live, |peak| peak.max(live)));

        let sample = ArbitrationRunSample {
            secs,
            drone_spawns: self.drone_spawns,
            live_agents: live,
        };
        let start = self.start_secs();
        let bucket = |at: f64| ((at - start) / SAMPLE_BUCKET_SECS).floor();
        match self.samples.last_mut() {
            Some(last) if secs < last.secs => {}
            Some(last) if (bucket(last.secs) - bucket(secs)).abs() < 0.5 => *last = sample,
            _ => self.samples.push(sample),
        }
    }

    pub(super) fn gameplay(&mut self, event: &LogEvent, secs: f64) {
        match event {
            LogEvent::SurvivalStarted => {
                self.gameplay_start_secs.get_or_insert(secs);
            }
            LogEvent::SurvivalReward { elapsed_secs } => self.survival_reward(secs, *elapsed_secs),
            LogEvent::RotationRewarded => self.rotation_rewarded(secs),
            LogEvent::DefenseWave { index } => self.wave(*index, secs),
            LogEvent::DefenseWaveCleared => self.wave_cleared(),
            LogEvent::AgentCreated {
                agent,
                live,
                spawned,
            } => self.agent(secs, agent, *live, *spawned),
            _ => {}
        }
    }

    pub(super) fn outcome(&mut self, event: &LogEvent, secs: f64) {
        match event {
            LogEvent::MissionSucceeded => {
                self.outcome_hint = Some(OutcomeHint {
                    status: ArbitrationRunStatus::Completed,
                    at_secs: secs,
                });
            }
            LogEvent::MissionFailed => {
                self.outcome_hint = Some(OutcomeHint {
                    status: ArbitrationRunStatus::Failed,
                    at_secs: secs,
                });
            }
            LogEvent::MissionAborted => self.end.record(secs, ArbitrationRunStatus::Aborted, true),
            LogEvent::PlayersExtracting => {
                self.end.record(secs, ArbitrationRunStatus::Completed, true);
            }
            LogEvent::MissionScore { node_id } => {
                self.node_id.get_or_insert_with(|| node_id.clone());
                let outcome = self.corroborated_outcome(secs);
                self.end.record(secs, outcome, false);
            }
            LogEvent::MissionLocationUnlocked => {
                let outcome = self.corroborated_outcome(secs);
                self.end.record(secs, outcome, false);
            }
            _ => {}
        }
    }

    fn reward_cycles(&self) -> Option<usize> {
        if let Some(last) = self.rotations.last() {
            return Some(last.index);
        }

        let per_rotation = if self.mirror_defense() {
            MIRROR_DEFENSE_WAVES_PER_ROTATION
        } else {
            WAVES_PER_ROTATION
        };
        match self.waves.first() {
            Some(first) if first.index == 1 && !self.capture_broken() => {
                let complete = self.waves.iter().filter(|wave| wave.complete).count();
                Some(complete / per_rotation)
            }
            None if self.gameplay_start_secs.is_some() && !self.capture_broken() => Some(0),
            Some(_) | None => None,
        }
    }

    fn capture_notes(&self) -> Vec<String> {
        let mut notes = self.notes.clone();
        if !self.started() {
            notes.insert(CaptureNote::NoStart);
        }
        if self.origin.is_none() {
            notes.insert(CaptureNote::NoClock);
        }
        notes.insert(if self.host() {
            CaptureNote::SpawnCounting
        } else {
            CaptureNote::ClientSide
        });
        notes
            .into_iter()
            .map(|note| note.text().to_owned())
            .collect()
    }

    pub(super) fn snapshot(
        &self,
        source: ArbitrationRunSource,
        boosters: &ArbitrationRunBoosterTracker,
    ) -> ArbitrationRun {
        let start = self.start_secs();
        let started_at = self
            .origin
            .zip(log_offset(start))
            .map(|(origin, offset)| origin + offset);
        let end = match self.end {
            ArbitrationMissionEnd::Ended { secs, .. } => secs,
            ArbitrationMissionEnd::Open | ArbitrationMissionEnd::Interrupted => self.last_secs,
        };
        let relative = |secs: f64| (secs - start).max(0.0);
        let host_telemetry = self.host_telemetry();
        let place = self
            .node_id
            .as_deref()
            .and_then(node_name)
            .and_then(node_and_planet);
        let origin_key = self.origin.map_or_else(
            || "unanchored".to_owned(),
            |origin| origin.timestamp_millis().to_string(),
        );
        let mut run = ArbitrationRun {
            id: format!("{origin_key}:{}", self.first_secs),
            source,
            status: self.end.status(),
            node_id: self.node_id.clone(),
            node: place.map(|(node, _)| node.to_owned()),
            planet: place.map(|(_, planet)| planet.to_owned()),
            mission_type: self.mission_type.clone(),
            started_at,
            duration_secs: (end - start).max(0.0),
            host: Some(self.host()),
            complete_capture: self.started() && self.end_observed() && !self.capture_broken(),
            notes: self.capture_notes(),
            rotations: self
                .rotations
                .iter()
                .map(|rotation| ArbitrationRunRotation {
                    at_secs: relative(rotation.at_secs),
                    ..*rotation
                })
                .collect(),
            waves: self.waves.clone(),
            reward_cycles: self.reward_cycles(),
            enemy_spawns: matches!(self.spawn_counter, SpawnCounter::Seen { .. })
                .then_some(self.enemy_spawns),
            drone_spawns: host_telemetry.then_some(self.drone_spawns),
            peak_live_agents: self.peak_live_agents,
            boosters: boosters.running_at_run_start(self.first_secs, start, started_at),
            samples: self
                .samples
                .iter()
                .map(|sample| ArbitrationRunSample {
                    secs: relative(sample.secs),
                    ..*sample
                })
                .collect(),
            rewards: None,
            collected_vitus: None,
            manual_vitus: None,
            vitus: None,
        };
        if let Some(rewards) = &self.rewards {
            run.attach_mission_end(rewards.clone(), self.collected_vitus);
        }

        run
    }
}
