use chrono::{DateTime, NaiveDateTime, Utc};
use wf_inventory::Booster;
use wf_log::{Event as LogEvent, LogLine, is_arbitration_drone};
use wf_worldstate::mission_type_name;

use super::active::{ActiveRun, CaptureNote, log_offset};
use super::boosters::ArbitrationRunBoosterTracker;
use super::{ArbitrationRun, ArbitrationRunReward, ArbitrationRunSource};

const CLOCK_RESTART_SECS: f64 = 300.0;
const LIVE_REPORT_INTERVAL_SECS: f64 = 2.0;

pub enum ArbitrationRunUpdate {
    Unchanged,
    Changed(Box<ArbitrationRun>),
    Finished(Box<ArbitrationRun>),
}

impl ArbitrationRunUpdate {
    fn finished(self) -> Option<ArbitrationRun> {
        match self {
            Self::Finished(run) => Some(*run),
            Self::Unchanged | Self::Changed(_) => None,
        }
    }
}

struct ReportedArbitrationRun {
    run: ArbitrationRun,
    secs: f64,
}

pub struct ArbitrationRunRecorder {
    source: ArbitrationRunSource,
    origin: Option<DateTime<Utc>>,
    previous_secs: Option<f64>,
    pending_node: Option<String>,
    pending_type: Option<String>,
    reported: Option<ReportedArbitrationRun>,
    boosters: ArbitrationRunBoosterTracker,
    active: Option<ActiveRun>,
}

fn parse_clock(utc: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(utc, "%a %b %e %H:%M:%S %Y")
        .ok()
        .map(|naive| naive.and_utc())
}

fn returned_home(event: &LogEvent) -> bool {
    match event {
        LogEvent::HubReturned => true,
        LogEvent::LevelLoaded { path } => {
            path.starts_with("/Lotus/Levels/Proc/PlayerShip/")
                || path.starts_with("/Lotus/Levels/Proc/Hub/")
                || path.starts_with("/Lotus/Levels/ClanDojo/")
        }
        _ => false,
    }
}

impl ArbitrationRunRecorder {
    pub fn new(source: ArbitrationRunSource) -> Self {
        Self {
            source,
            origin: None,
            previous_secs: None,
            pending_node: None,
            pending_type: None,
            reported: None,
            boosters: ArbitrationRunBoosterTracker::default(),
            active: None,
        }
    }

    pub fn keep_latest_booster_expiries(&mut self, boosters: &[Booster]) {
        self.boosters.keep_latest_booster_expiries(boosters);
    }

    pub fn live(&self) -> Option<ArbitrationRun> {
        self.active.as_ref().map(|active| self.snapshot(active))
    }

    pub fn attach_mission_end(
        &mut self,
        rewards: &[ArbitrationRunReward],
        collected_vitus: Option<u32>,
    ) -> bool {
        self.active
            .as_mut()
            .is_some_and(|active| active.attach_mission_end(rewards, collected_vitus))
    }

    pub fn finish(&mut self) -> ArbitrationRunUpdate {
        self.interrupt(CaptureNote::LogEnd)
            .map_or(ArbitrationRunUpdate::Unchanged, Self::close_update)
    }

    pub fn replay(lines: &[LogLine], source: ArbitrationRunSource) -> Vec<ArbitrationRun> {
        let mut recorder = Self::new(source);
        let mut runs: Vec<ArbitrationRun> = lines
            .iter()
            .filter_map(|line| {
                let event = wf_log::classify(line)?;
                recorder.observe(&event, line.time, None).finished()
            })
            .collect();
        runs.extend(recorder.finish().finished());
        runs
    }

    fn close_update(run: ArbitrationRun) -> ArbitrationRunUpdate {
        if run.rewarded() {
            ArbitrationRunUpdate::Finished(Box::new(run))
        } else {
            ArbitrationRunUpdate::Changed(Box::new(run))
        }
    }

    fn snapshot(&self, active: &ActiveRun) -> ArbitrationRun {
        active.snapshot(self.source, &self.boosters)
    }

    fn closed(&mut self, active: &ActiveRun) -> ArbitrationRun {
        self.pending_node = None;
        self.pending_type = None;
        self.reported = None;
        self.snapshot(active)
    }

    fn report_due(&self, run: &ArbitrationRun, secs: f64) -> bool {
        let Some(reported) = &self.reported else {
            return true;
        };

        let mut seen = reported.run.clone();
        seen.duration_secs = run.duration_secs;
        if secs - reported.secs < LIVE_REPORT_INTERVAL_SECS {
            seen.samples.clone_from(&run.samples);
            seen.enemy_spawns = run.enemy_spawns;
            seen.drone_spawns = run.drone_spawns;
            seen.peak_live_agents = run.peak_live_agents;
        }

        seen != *run
    }

    fn close_active(&mut self) -> Option<ArbitrationRun> {
        let active = self.active.take()?;
        Some(self.closed(&active))
    }

    fn interrupt(&mut self, note: CaptureNote) -> Option<ArbitrationRun> {
        self.active.as_mut()?.interrupt(note);
        self.close_active()
    }

    fn start(&mut self, secs: f64, current_node: Option<&str>) {
        let node_id = self
            .pending_node
            .clone()
            .or_else(|| current_node.map(str::to_owned));
        let mission_type = self.pending_type.as_deref().map(mission_type_name);
        self.active = Some(ActiveRun::new(secs, node_id, mission_type, self.origin));
    }

    fn clock_restart(&mut self, event: &LogEvent, secs: f64) -> Option<ArbitrationRun> {
        let restarted = self.previous_secs.is_some_and(|previous| {
            secs < previous - 1.0
                && (matches!(event, LogEvent::ClockAnchor { .. })
                    || secs < previous - CLOCK_RESTART_SECS)
        });
        self.previous_secs = Some(secs);
        if !restarted {
            return None;
        }

        let finished = self.interrupt(CaptureNote::ClockRestart);
        self.origin = None;
        self.boosters.forget_logged();
        finished
    }

    fn node_seen(
        &mut self,
        node_id: &str,
        elite: bool,
        loading: bool,
        secs: f64,
    ) -> Option<ArbitrationRun> {
        let next_mission = loading
            && self.active.as_ref().is_some_and(|active| {
                active.started() && !active.end_observed() && active.node_id() != Some(node_id)
            });
        let finished = if next_mission {
            self.interrupt(CaptureNote::NextMission)
        } else {
            None
        };

        self.pending_node = Some(node_id.to_owned());
        if let Some(active) = self.active.as_mut() {
            if elite {
                active.set_node(node_id);
            }
        } else if elite && loading {
            self.start(secs, None);
        }

        finished
    }

    fn mission_context(
        &mut self,
        event: &LogEvent,
        secs: f64,
        current_node: Option<&str>,
    ) -> Option<ArbitrationRun> {
        let next_mission_line = matches!(
            event,
            LogEvent::MissionPending { .. }
                | LogEvent::MissionLoading { .. }
                | LogEvent::MissionSet {
                    node_id: Some(_),
                    ..
                }
        );
        let ended =
            if next_mission_line && self.active.as_ref().is_some_and(ActiveRun::end_observed) {
                self.close_active()
            } else {
                None
            };

        let interrupted = match event {
            LogEvent::MissionPending { node_id, elite } => {
                self.node_seen(node_id, *elite, false, secs)
            }
            LogEvent::MissionLoading {
                host,
                node_id,
                elite,
            } => {
                let interrupted = self.node_seen(node_id, *elite, true, secs);
                if let Some(active) = self.active.as_mut() {
                    active.set_host(*host);
                }
                interrupted
            }
            LogEvent::MissionSet {
                node_id: Some(node_id),
                elite,
                ..
            } => self.node_seen(node_id, *elite, false, secs),
            LogEvent::MissionStateStarted { mission_type } => {
                self.pending_type = Some(mission_type.clone());
                if let Some(active) = self.active.as_mut() {
                    active.set_mission_type(mission_type_name(mission_type));
                }
                None
            }
            LogEvent::AgentCreated { agent, .. }
                if self.active.is_none() && is_arbitration_drone(agent) =>
            {
                self.start(secs, current_node);
                None
            }
            _ => None,
        };
        ended.or(interrupted)
    }

    fn session_context(
        &mut self,
        event: &LogEvent,
        secs: f64,
        current_node: Option<&str>,
    ) -> Option<ArbitrationRun> {
        let restarted = self.clock_restart(event, secs);
        self.boosters.observe(event, secs);
        if let LogEvent::ClockAnchor { utc } = event {
            self.origin = parse_clock(utc)
                .zip(log_offset(secs))
                .map(|(clock, offset)| clock - offset);
        }

        let finished = self.mission_context(event, secs, current_node);
        restarted.or(finished)
    }

    pub fn observe(
        &mut self,
        event: &LogEvent,
        secs: f64,
        current_node: Option<&str>,
    ) -> ArbitrationRunUpdate {
        if let Some(finished) = self.session_context(event, secs, current_node) {
            return Self::close_update(finished);
        }

        let Some(mut active) = self.active.take() else {
            return ArbitrationRunUpdate::Unchanged;
        };

        active.advance(secs);
        match event {
            LogEvent::MissionStateStarted { .. } => active.mission_started(secs),
            LogEvent::SessionStarted { host } => active.set_host(*host),
            _ if !active.end_observed() => active.gameplay(event, secs),
            _ => {}
        }

        active.outcome(event, secs);
        if active.end_observed() && returned_home(event) {
            return Self::close_update(self.closed(&active));
        }

        let run = self.snapshot(&active);
        self.active = Some(active);
        if !self.report_due(&run, secs) {
            return ArbitrationRunUpdate::Unchanged;
        }

        self.reported = Some(ReportedArbitrationRun {
            run: run.clone(),
            secs,
        });
        ArbitrationRunUpdate::Changed(Box::new(run))
    }
}

#[cfg(test)]
pub(crate) fn fixture_lines() -> Vec<LogLine> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/arbitration_survival.log"
    );
    wf_log::read_all(std::path::Path::new(path)).unwrap()
}

#[cfg(test)]
pub(crate) fn fixture_runs() -> Vec<ArbitrationRun> {
    ArbitrationRunRecorder::replay(&fixture_lines(), ArbitrationRunSource::Import)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arbitration_runs::boosters::inventory_booster;
    use crate::arbitration_runs::{
        ArbitrationRunResourceBooster, ArbitrationRunResourceBoosterKind, ArbitrationRunStatus,
    };

    #[test]
    fn survival_fixture_run() {
        let runs = fixture_runs();
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert_eq!(run.status, ArbitrationRunStatus::Completed);
        assert_eq!(run.node_id.as_deref(), Some("ClanNode11"));
        assert_eq!(run.node.as_deref(), Some("Zeugma"));
        assert_eq!(run.planet.as_deref(), Some("Phobos"));
        assert_eq!(run.mission_type.as_deref(), Some("Survival"));
        assert_eq!(run.host, Some(true));
        assert!(run.complete_capture);
        assert_eq!(run.source, ArbitrationRunSource::Import);
        assert_eq!(run.id, "1792858804954:16468.857");
        assert_eq!(
            run.started_at.unwrap().to_rfc3339(),
            "2026-10-24T20:54:38.050+00:00"
        );
        assert!((run.duration_secs - 333.881).abs() < 0.01);
        assert_eq!(run.rotations.len(), 1);
        assert_eq!(run.rotations[0].index, 1);
        assert!((run.rotations[0].at_secs - 313.187).abs() < 0.01);
        assert!((run.rotations[0].duration_secs.unwrap() - 300.495).abs() < 0.01);
        assert_eq!(run.reward_cycles, Some(1));
        assert_eq!(run.drone_spawns, Some(7));
        assert_eq!(run.enemy_spawns, Some(438));
        assert_eq!(run.peak_live_agents, Some(17));
        assert_eq!(run.samples.len(), 65);
        assert!(run.waves.is_empty());
        assert!(run.boosters.is_empty());
        assert!(
            run.samples
                .windows(2)
                .all(|pair| pair[0].secs < pair[1].secs)
        );
        assert_eq!(run.notes, [CaptureNote::SpawnCounting.text()]);
    }

    fn arbitration_start() -> Vec<(LogEvent, f64)> {
        vec![
            (
                LogEvent::ClockAnchor {
                    utc: "Sun Oct 4 16:20:05 2026".to_owned(),
                },
                1.0,
            ),
            (
                LogEvent::MissionPending {
                    node_id: "SolNode64".to_owned(),
                    elite: true,
                },
                10.0,
            ),
            (
                LogEvent::MissionLoading {
                    host: false,
                    node_id: "SolNode64".to_owned(),
                    elite: true,
                },
                12.0,
            ),
            (
                LogEvent::MissionStateStarted {
                    mission_type: "MT_TERRITORY".to_owned(),
                },
                20.0,
            ),
        ]
    }

    fn drive(events: &[(LogEvent, f64)]) -> (Vec<ArbitrationRun>, ArbitrationRunRecorder) {
        let mut recorder = ArbitrationRunRecorder::new(ArbitrationRunSource::Live);
        let mut runs = Vec::new();
        for (event, secs) in events {
            runs.extend(recorder.observe(event, *secs, None).finished());
        }
        (runs, recorder)
    }

    fn ended(update: &ArbitrationRunUpdate) -> bool {
        matches!(update, ArbitrationRunUpdate::Changed(run) if run.status != ArbitrationRunStatus::Recording)
    }

    #[test]
    fn late_join_start() {
        let events = vec![
            (
                LogEvent::MissionPending {
                    node_id: "SolNode450".to_owned(),
                    elite: true,
                },
                10.0,
            ),
            (
                LogEvent::MissionLoading {
                    host: false,
                    node_id: "SolNode450".to_owned(),
                    elite: true,
                },
                15.0,
            ),
            (
                LogEvent::MissionStateStarted {
                    mission_type: "MT_DEFENSE".to_owned(),
                },
                15.2,
            ),
            (LogEvent::RotationRewarded, 300.0),
        ];
        let (runs, recorder) = drive(&events);
        assert!(runs.is_empty());
        let live = recorder.live().unwrap();
        assert_eq!(live.node.as_deref(), Some("Tyana Pass"));
        assert_eq!(live.planet.as_deref(), Some("Mars"));
        assert_eq!(live.node_id.as_deref(), Some("SolNode450"));
        assert_eq!(live.host, Some(false));
        assert_eq!(live.reward_cycles, Some(1));
        assert_eq!(live.drone_spawns, None);
        assert!(
            live.notes
                .iter()
                .any(|note| note.starts_with("Recorded as a client"))
        );
    }

    #[test]
    fn aborted_run() {
        let mut events = arbitration_start();
        events.push((LogEvent::RotationRewarded, 80.0));
        events.push((LogEvent::MissionAborted, 90.0));
        events.push((
            LogEvent::LevelLoaded {
                path: "/Lotus/Levels/Proc/PlayerShip/AeAo.lp".to_owned(),
            },
            100.0,
        ));
        let (runs, _) = drive(&events);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, ArbitrationRunStatus::Aborted);
        assert_eq!(runs[0].mission_type.as_deref(), Some("Interception"));
        assert_eq!(runs[0].host, Some(false));
        assert!((runs[0].duration_secs - 70.0).abs() < f64::EPSILON);
        assert_eq!(
            runs[0].started_at.unwrap().to_rfc3339(),
            "2026-10-04T16:20:24+00:00"
        );
        assert_eq!(runs[0].reward_cycles, Some(1));
    }

    #[test]
    fn unrewarded_run_dropped() {
        let mut events = arbitration_start();
        events.push((LogEvent::MissionAborted, 90.0));
        events.push((
            LogEvent::LevelLoaded {
                path: "/Lotus/Levels/Proc/PlayerShip/AeAo.lp".to_owned(),
            },
            100.0,
        ));
        let mut recorder = ArbitrationRunRecorder::new(ArbitrationRunSource::Live);
        let mut last = ArbitrationRunUpdate::Unchanged;
        for (event, secs) in &events {
            last = recorder.observe(event, *secs, None);
        }

        assert!(
            matches!(last, ArbitrationRunUpdate::Changed(run) if run.status == ArbitrationRunStatus::Aborted)
        );
        assert!(recorder.live().is_none());
        assert!(matches!(
            drive(&arbitration_start()).1.finish(),
            ArbitrationRunUpdate::Changed(_)
        ));
    }

    #[test]
    fn failed_outcome_from_score() {
        let mut events = arbitration_start();
        events.push((LogEvent::MissionFailed, 200.0));
        events.push((
            LogEvent::MissionScore {
                node_id: "SolNode64".to_owned(),
            },
            205.0,
        ));
        let (runs, recorder) = drive(&events);
        assert!(runs.is_empty());
        let live = recorder.live().unwrap();
        assert_eq!(live.status, ArbitrationRunStatus::Failed);
        assert!(live.complete_capture);
    }

    #[test]
    fn next_mission_interrupts() {
        let mut events = arbitration_start();
        events.push((LogEvent::RotationRewarded, 30.0));
        events.push((
            LogEvent::MissionLoading {
                host: true,
                node_id: "SolNode100".to_owned(),
                elite: false,
            },
            400.0,
        ));
        let (runs, recorder) = drive(&events);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, ArbitrationRunStatus::Interrupted);
        assert!(
            runs[0]
                .notes
                .contains(&CaptureNote::NextMission.text().to_owned())
        );
        assert!(!runs[0].complete_capture);
        assert!(recorder.live().is_none());
    }

    #[test]
    fn clock_restart_and_log_end() {
        let mut events = arbitration_start();
        events.push((LogEvent::RotationRewarded, 320.0));
        events.push((
            LogEvent::ClockAnchor {
                utc: "Sun Oct 4 17:00:00 2026".to_owned(),
            },
            0.5,
        ));
        events.push((
            LogEvent::MissionLoading {
                host: true,
                node_id: "SolNode64".to_owned(),
                elite: true,
            },
            30.0,
        ));
        let (runs, mut recorder) = drive(&events);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, ArbitrationRunStatus::Interrupted);
        assert!(
            runs[0]
                .notes
                .contains(&CaptureNote::ClockRestart.text().to_owned())
        );
        assert_eq!(
            recorder.live().unwrap().started_at.unwrap().to_rfc3339(),
            "2026-10-04T17:00:29.500+00:00"
        );
        recorder.observe(&LogEvent::RotationRewarded, 330.0, None);
        let run = recorder.finish().finished().unwrap();
        assert_eq!(run.status, ArbitrationRunStatus::Interrupted);
        assert!(run.notes.contains(&CaptureNote::LogEnd.text().to_owned()));
    }

    #[test]
    fn defense_wave_cycles() {
        let mut events = arbitration_start();
        for (index, secs) in (1..=7).zip([30.0, 90.0, 150.0, 210.0, 270.0, 330.0, 390.0]) {
            events.push((LogEvent::DefenseWave { index }, secs));
        }
        events.push((LogEvent::DefenseWaveCleared, 440.0));
        let (_, recorder) = drive(&events);
        let live = recorder.live().unwrap();
        assert_eq!(live.waves.len(), 7);
        assert!(live.waves.iter().all(|wave| wave.complete));
        assert_eq!(live.reward_cycles, Some(2));
    }

    #[test]
    fn blessing_expiry_merge() {
        let mut events = arbitration_start();
        events.insert(1, (LogEvent::HubBlessingSent { resource: true }, 2.0));
        events.insert(
            2,
            (
                LogEvent::HubBlessingConfirmed {
                    resource: true,
                    duration_secs: 500.0,
                },
                3.0,
            ),
        );
        let (_, mut recorder) = drive(&events);
        let blessing = |expires_secs| {
            vec![ArbitrationRunResourceBooster {
                kind: ArbitrationRunResourceBoosterKind::DropChanceBlessing,
                expires_secs,
            }]
        };
        assert_eq!(recorder.live().unwrap().boosters, blessing(502.0 - 20.0));

        recorder.keep_latest_booster_expiries(&[inventory_booster(
            ArbitrationRunResourceBoosterKind::DropChanceBlessing,
            1_791_130_824 + 1_000,
        )]);
        assert_eq!(recorder.live().unwrap().boosters, blessing(1_000.0));
    }

    #[test]
    fn dojo_closes_ended_run() {
        let mut events = arbitration_start();
        events.push((LogEvent::RotationRewarded, 300.0));
        events.push((LogEvent::PlayersExtracting, 400.0));
        events.push((
            LogEvent::LevelLoaded {
                path: "/Lotus/Levels/ClanDojo/Dojo.lp".to_owned(),
            },
            420.0,
        ));
        let (runs, recorder) = drive(&events);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, ArbitrationRunStatus::Completed);
        assert!(recorder.live().is_none());
    }

    #[test]
    fn open_level_keeps_client_role() {
        let mut events = arbitration_start();
        events.push((
            LogEvent::MissionStart {
                level_path: "/Lotus/Levels/Proc/Grineer/GrineerSettlementSurvival.lp".to_owned(),
            },
            21.0,
        ));
        let (_, recorder) = drive(&events);
        assert_eq!(recorder.live().unwrap().host, Some(false));
    }

    #[test]
    fn live_id_stable() {
        let events = arbitration_start();
        let mut recorder = ArbitrationRunRecorder::new(ArbitrationRunSource::Live);
        let mut ids = Vec::new();
        for (event, secs) in &events {
            recorder.observe(event, *secs, None);
            ids.extend(recorder.live().map(|run| run.id));
        }
        ids.dedup();
        assert_eq!(ids, vec!["1791130804000:12"]);
    }

    #[test]
    fn drone_start_counts_as_host() {
        let mut recorder = ArbitrationRunRecorder::new(ArbitrationRunSource::Live);
        let drone = LogEvent::AgentCreated {
            agent: "/Npc/CorpusEliteShieldDroneAgent".to_owned(),
            live: 3,
            spawned: 2,
        };
        recorder.observe(&drone, 12.0, Some("SolNode64"));

        let run = recorder.live().unwrap();
        assert_eq!(run.host, Some(true));
        assert!(
            !run.notes
                .contains(&CaptureNote::ClientSide.text().to_owned())
        );
    }

    #[test]
    fn live_updates_throttled() {
        let (_, mut recorder) = drive(&arbitration_start());
        let agent = |spawned| LogEvent::AgentCreated {
            agent: "/Npc/GrineerLancer".to_owned(),
            live: spawned,
            spawned,
        };
        assert!(matches!(
            recorder.observe(&agent(1), 20.5, None),
            ArbitrationRunUpdate::Unchanged
        ));
        assert!(matches!(
            recorder.observe(&agent(2), 23.0, None),
            ArbitrationRunUpdate::Changed(_)
        ));
        assert!(matches!(
            recorder.observe(&LogEvent::SurvivalStarted, 23.5, None),
            ArbitrationRunUpdate::Changed(_)
        ));
        assert!(matches!(
            recorder.observe(&LogEvent::InventorySynced, 30.0, None),
            ArbitrationRunUpdate::Unchanged
        ));
    }

    #[test]
    fn reward_cycle_deduped() {
        let mut events = arbitration_start();
        events.push((LogEvent::SurvivalStarted, 25.0));
        events.push((LogEvent::RotationRewarded, 325.0));
        events.push((
            LogEvent::SurvivalReward {
                elapsed_secs: 300.0,
            },
            326.0,
        ));
        let (_, recorder) = drive(&events);
        assert_eq!(recorder.live().unwrap().rotations.len(), 1);
    }

    #[test]
    fn spawn_counter_reset() {
        let mut events = arbitration_start();
        let agent = |spawned: u32, secs: f64| {
            (
                LogEvent::AgentCreated {
                    agent: format!("/Npc/MeleeRunner{spawned}"),
                    live: 1,
                    spawned,
                },
                secs,
            )
        };
        events.extend([
            agent(10, 30.0),
            agent(14, 31.0),
            agent(2, 40.0),
            agent(5, 41.0),
        ]);
        let (_, recorder) = drive(&events);
        let live = recorder.live().unwrap();
        assert_eq!(live.enemy_spawns, Some(7));
        assert!(
            live.notes
                .contains(&CaptureNote::CounterReset.text().to_owned())
        );
        assert!(!live.complete_capture);
    }

    #[test]
    fn client_rotations() {
        let mut events = arbitration_start();
        events.push((LogEvent::SurvivalStarted, 25.0));
        events.push((
            LogEvent::SurvivalReward {
                elapsed_secs: 300.49,
            },
            325.5,
        ));
        events.push((
            LogEvent::SurvivalReward {
                elapsed_secs: 600.98,
            },
            626.0,
        ));
        let (_, recorder) = drive(&events);
        let live = recorder.live().unwrap();
        assert_eq!(live.reward_cycles, Some(2));
    }

    #[test]
    fn next_run_after_end() {
        let mut events = arbitration_start();
        events.push((LogEvent::RotationRewarded, 300.0));
        events.push((LogEvent::PlayersExtracting, 400.0));
        events.push((
            LogEvent::MissionScore {
                node_id: "SolNode64".to_owned(),
            },
            401.0,
        ));
        events.push((
            LogEvent::MissionLoading {
                host: false,
                node_id: "SolNode450".to_owned(),
                elite: true,
            },
            430.0,
        ));
        let (runs, recorder) = drive(&events);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, ArbitrationRunStatus::Completed);
        assert_eq!(
            recorder.live().unwrap().node_id.as_deref(),
            Some("SolNode450")
        );
    }

    #[test]
    fn extraction_ends_update() {
        let mut recorder = ArbitrationRunRecorder::new(ArbitrationRunSource::Live);
        for (event, secs) in arbitration_start() {
            assert!(!ended(&recorder.observe(&event, secs, None)));
        }
        assert!(ended(&recorder.observe(
            &LogEvent::PlayersExtracting,
            400.0,
            None
        )));
        assert!(!ended(&recorder.observe(
            &LogEvent::MissionLocationUnlocked,
            401.0,
            None
        )));
    }

    #[test]
    fn clock_single_digit_day() {
        assert_eq!(
            parse_clock("Sun Oct  4 16:20:05 2026")
                .unwrap()
                .to_rfc3339(),
            "2026-10-04T16:20:05+00:00"
        );
        assert!(parse_clock("Sat Oct 4 16:20:05 2026").is_none());
        assert!(parse_clock("yesterday").is_none());
    }
}
