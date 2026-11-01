//! Transparent, click-through overlay that draws active buffs as arcs around the reticle.
//!
//!   wf-overlay [--demo]

mod focus;
mod ring;
mod source;
mod tracker;

use std::time::{Duration, Instant};

use eframe::egui::{self, Vec2};

use ring::{LABEL_MARGIN, Style};
use source::{BuffSource, Demo, Pending};
use tracker::Tracker;

const STYLE: Style = Style {
    radius: 70.0,
    thickness: 5.0,
    warn_secs: 4.0,
};
/// Seconds an expired buff keeps pulsing on the ring.
const LINGER: f64 = 4.0;
const FOCUS_TITLE: &str = "Warframe";

fn main() -> eframe::Result {
    let demo = std::env::args().any(|a| a == "--demo");
    let source: Box<dyn BuffSource> = if demo { Box::new(Demo) } else { Box::new(Pending) };

    let side = 2.0 * (STYLE.radius + LABEL_MARGIN);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("wf-overlay")
            .with_inner_size([side, side])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_mouse_passthrough(true)
            .with_taskbar(false)
            .with_resizable(false),
        ..Default::default()
    };
    let app = App {
        source,
        tracker: Tracker::new(LINGER),
        start: Instant::now(),
        side,
        centered: false,
        demo,
    };
    eframe::run_native("wf-overlay", options, Box::new(|_| Ok(Box::new(app))))
}

struct App {
    source: Box<dyn BuffSource>,
    tracker: Tracker,
    start: Instant,
    side: f32,
    centered: bool,
    demo: bool,
}

impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(33));

        if !self.centered
            && let Some(monitor) = ctx.input(|i| i.viewport().monitor_size)
        {
            let pos = (monitor - Vec2::splat(self.side)) / 2.0;
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos.to_pos2()));
            self.centered = true;
        }

        let now = self.start.elapsed().as_secs_f64();
        let readings = self.source.poll(now);
        self.tracker.update(&readings, now);

        let visible = self.demo || focus::foreground_title_contains(FOCUS_TITLE);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                if visible {
                    let status = self.source.status();
                    ring::draw(
                        ui.painter(),
                        ui.max_rect().center(),
                        &STYLE,
                        &self.tracker.shown(),
                        status.as_deref(),
                        now,
                    );
                }
            });
    }
}
