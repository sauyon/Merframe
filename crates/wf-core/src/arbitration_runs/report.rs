use std::fmt::Write as _;

use super::{ArbitrationRun, ArbitrationRunStatus};

const SVG_WIDTH: usize = 1080;
const SVG_LINE_HEIGHT: usize = 30;

fn status_text(status: ArbitrationRunStatus) -> &'static str {
    match status {
        ArbitrationRunStatus::Recording => "recording",
        ArbitrationRunStatus::Completed => "completed",
        ArbitrationRunStatus::Failed => "failed",
        ArbitrationRunStatus::Aborted => "aborted",
        ArbitrationRunStatus::Ended => "ended",
        ArbitrationRunStatus::Interrupted => "interrupted",
    }
}

fn or_unknown(value: Option<impl ToString>) -> String {
    value.map_or_else(|| String::from("unknown"), |value| value.to_string())
}

pub fn text_report(run: &ArbitrationRun) -> String {
    let mut text = String::from("Arbitration run report\n");
    let _ = writeln!(text, "{}", run.place());
    let _ = writeln!(
        text,
        "Mission: {}",
        run.mission_type.as_deref().unwrap_or("unknown")
    );
    let _ = writeln!(
        text,
        "Started: {}",
        run.started_at
            .map_or_else(|| String::from("unknown"), |at| at.to_rfc3339())
    );
    let _ = writeln!(text, "Duration: {:.0} seconds", run.duration_secs);
    let _ = writeln!(
        text,
        "Outcome: {}, capture: {}",
        status_text(run.status),
        if run.complete_capture {
            "complete"
        } else {
            "partial"
        }
    );
    let _ = writeln!(
        text,
        "Role: {}",
        run.host
            .map_or("unknown", |host| if host { "host" } else { "client" })
    );
    let _ = writeln!(text, "Reward cycles: {}", or_unknown(run.reward_cycles));
    let _ = writeln!(
        text,
        "Waves completed: {}",
        run.waves.iter().filter(|wave| wave.complete).count()
    );
    let _ = writeln!(
        text,
        "Observed enemy spawns: {}",
        or_unknown(run.enemy_spawns)
    );
    let _ = writeln!(
        text,
        "Observed drone spawns: {}",
        or_unknown(run.drone_spawns)
    );
    if let Some(vitus) = run.vitus {
        let _ = writeln!(text, "Vitus Essence: {vitus}");
    }
    for note in &run.notes {
        let _ = writeln!(text, "Capture note: {note}");
    }
    text.push_str("Recorded with Merframe\n");
    text
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn svg_report(run: &ArbitrationRun) -> String {
    let text = text_report(run);
    let lines: Vec<&str> = text.lines().collect();
    let height = (80 + lines.len() * SVG_LINE_HEIGHT).max(400);
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{SVG_WIDTH}" height="{height}" viewBox="0 0 {SVG_WIDTH} {height}"><rect width="100%" height="100%" fill="#1c1917"/>"##
    );
    for (index, line) in lines.iter().enumerate() {
        let (fill, size) = if index == 0 {
            ("#f5d060", 28)
        } else {
            ("#f5f1ea", 17)
        };
        let _ = write!(
            svg,
            r#"<text x="36" y="{}" fill="{fill}" font-family="sans-serif" font-size="{size}">{}</text>"#,
            48 + index * SVG_LINE_HEIGHT,
            escape(line)
        );
    }
    svg.push_str("</svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arbitration_runs::ArbitrationRunSource;

    fn run() -> ArbitrationRun {
        ArbitrationRun {
            id: String::from("1:ClanNode11:1"),
            source: ArbitrationRunSource::Live,
            status: ArbitrationRunStatus::Completed,
            node_id: Some(String::from("ClanNode11")),
            node: Some(String::from("Zeugma")),
            planet: Some(String::from("Phobos")),
            mission_type: Some(String::from("Survival")),
            started_at: None,
            duration_secs: 333.9,
            host: Some(true),
            complete_capture: true,
            notes: vec![String::from("A note")],
            rotations: Vec::new(),
            waves: Vec::new(),
            reward_cycles: Some(1),
            enemy_spawns: Some(438),
            drone_spawns: Some(7),
            peak_live_agents: Some(17),
            boosters: Vec::new(),
            samples: Vec::new(),
            rewards: None,
            collected_vitus: Some(3),
            manual_vitus: Some(4),
            vitus: Some(4),
        }
    }

    #[test]
    fn text_report_lines() {
        let text = text_report(&run());
        assert!(text.starts_with("Arbitration run report\nZeugma, Phobos\nMission: Survival\n"));
        assert!(text.contains("Started: unknown\n"));
        assert!(text.contains("Duration: 334 seconds\n"));
        assert!(text.contains("Outcome: completed, capture: complete\n"));
        assert!(text.contains("Role: host\n"));
        assert!(text.contains("Observed drone spawns: 7\n"));
        assert!(text.contains("Vitus Essence: 4\n"));
        assert!(text.contains("Capture note: A note\n"));
        assert!(text.ends_with("Recorded with Merframe\n"));
    }

    #[test]
    fn svg_report_escapes_and_sizes() {
        let mut noted = run();
        noted.notes = vec![String::from("a < b & c")];
        let svg = svg_report(&noted);
        let height = 80 + text_report(&noted).lines().count() * 30;
        assert!(svg.starts_with(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1080\" height=\"{height}\""
        )));
        assert!(svg.contains("a &lt; b &amp; c"));
        assert!(svg.ends_with("</svg>"));
    }
}
