//! Draws one arc per buff around the screen centre.

use std::f32::consts::{PI, TAU};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Shape, Stroke, Vec2, ecolor::Hsva};

use crate::tracker::Shown;

pub struct Style {
    pub radius: f32,
    pub thickness: f32,
    /// Arc turns amber below this many seconds.
    pub warn_secs: f64,
}

/// Room outside the ring for labels.
pub const LABEL_MARGIN: f32 = 70.0;
const ARC_GAP: f32 = 0.18;
const WARN: Color32 = Color32::from_rgb(255, 170, 40);
const DOWN: Color32 = Color32::from_rgb(255, 60, 60);

pub fn draw(p: &egui::Painter, c: Pos2, style: &Style, buffs: &[Shown], status: Option<&str>, now: f64) {
    #[allow(clippy::cast_precision_loss, reason = "a handful of buffs")]
    let span = TAU / buffs.len().max(1) as f32;
    for (i, b) in buffs.iter().enumerate() {
        #[allow(clippy::cast_precision_loss, reason = "a handful of buffs")]
        let a0 = -PI / 2.0 + i as f32 * span + ARC_GAP / 2.0;
        let a1 = a0 + span - ARC_GAP;
        let base = label_color(&b.label);
        let (color, track) = match b.remaining {
            Some(r) if r < style.warn_secs => (WARN, base.gamma_multiply(0.2)),
            Some(_) => (base, base.gamma_multiply(0.2)),
            None => {
                #[allow(clippy::cast_possible_truncation, reason = "only drives a pulse")]
                let pulse = 0.55 + 0.45 * (now as f32 * 7.0).sin();
                (DOWN.gamma_multiply(pulse), DOWN.gamma_multiply(0.35 * pulse))
            }
        };

        arc(p, c, style.radius, a0, a1, Stroke::new(style.thickness, track));
        if let Some(r) = b.remaining {
            #[allow(clippy::cast_possible_truncation, reason = "a fraction in [0, 1]")]
            let frac = (r / b.total.max(f64::EPSILON)).clamp(0.0, 1.0) as f32;
            arc(p, c, style.radius, a0, a0 + (a1 - a0) * frac, Stroke::new(style.thickness, color));
        }

        let at = c + Vec2::angled(a0.midpoint(a1)) * (style.radius + style.thickness + 22.0);
        let label = match b.remaining {
            Some(r) => format!("{}\n{:.0}", b.label, r.ceil()),
            None => b.label.clone(),
        };
        text(p, at, &label, 13.0, color);
    }

    if let Some(s) = status {
        text(p, c + Vec2::new(0.0, style.radius * 0.45), s, 12.0, Color32::from_gray(230));
    }
}

/// A stable colour per buff name, so the same buff always looks the same.
fn label_color(label: &str) -> Color32 {
    let hash = label
        .bytes()
        .fold(0x811c_9dc5_u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    #[allow(clippy::cast_precision_loss, reason = "only picks a hue")]
    let hue = (hash % 360) as f32 / 360.0;
    Hsva::new(hue, 0.55, 1.0, 1.0).into()
}

fn arc(p: &egui::Painter, c: Pos2, r: f32, a0: f32, a1: f32, stroke: Stroke) {
    if a1 <= a0 {
        return;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "small positive step count"
    )]
    let steps = ((a1 - a0) / 0.05).ceil().max(2.0) as usize;
    #[allow(clippy::cast_precision_loss, reason = "small step count")]
    let pts = (0..=steps)
        .map(|s| c + Vec2::angled(a0 + (a1 - a0) * s as f32 / steps as f32) * r)
        .collect();
    p.add(Shape::line(pts, stroke));
}

/// Text with a dark outline so it reads over any scene.
fn text(p: &egui::Painter, at: Pos2, s: &str, size: f32, color: Color32) {
    let font = FontId::proportional(size);
    let shadow = Color32::from_black_alpha(200);
    for d in [
        Vec2::new(1.0, 1.0),
        Vec2::new(-1.0, 1.0),
        Vec2::new(1.0, -1.0),
        Vec2::new(-1.0, -1.0),
    ] {
        p.text(at + d, Align2::CENTER_CENTER, s, font.clone(), shadow);
    }
    p.text(at, Align2::CENTER_CENTER, s, font, color);
}
