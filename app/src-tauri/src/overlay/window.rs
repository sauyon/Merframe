use std::path::PathBuf;

use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalPosition, Runtime, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};
use tracing::{debug, warn};
use wf_log::MonitorRect;

use super::{Kind, placement_of};
use crate::settings::{OverlayPlacement, Settings, TextSize};

const MARGIN: f64 = 20.0;

const DEFAULT_SCREEN: Screen = Screen {
    x: 0.0,
    y: 0.0,
    width: 1920.0,
    height: 1080.0,
    scale: 1.0,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Screen {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) scale: f64,
}

impl Screen {
    fn from_monitor(monitor: &tauri::Monitor) -> Self {
        let position = monitor.position();
        let size = monitor.size();
        Self {
            x: f64::from(position.x),
            y: f64::from(position.y),
            width: f64::from(size.width),
            height: f64::from(size.height),
            scale: monitor.scale_factor(),
        }
    }

    /// Wine moves the primary monitor to 0,0,
    /// but the physical position reports the absolute coordinates.
    fn from_game(rect: MonitorRect, primary_origin: PhysicalPosition<i32>, scale: f64) -> Self {
        Self {
            x: f64::from(rect.left + primary_origin.x),
            y: f64::from(rect.top + primary_origin.y),
            width: f64::from(rect.width),
            height: f64::from(rect.height),
            scale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Bounds {
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) x: f64,
    pub(super) y: f64,
}

pub(super) fn logical(bounds: Bounds, scale: f64) -> Bounds {
    Bounds {
        width: bounds.width / scale,
        height: bounds.height / scale,
        x: bounds.x / scale,
        y: bounds.y / scale,
    }
}

pub(super) fn window_bounds(screen: Screen, kind: Kind, settings: &Settings) -> Bounds {
    let rows = settings.overlays.overlay_recommendation_count;
    logical(
        bounds_for(
            kind,
            placement_of(settings, kind),
            rows,
            settings.text_size,
            screen,
        ),
        screen.scale,
    )
}

pub(super) fn bounds_for(
    kind: Kind,
    placement: OverlayPlacement,
    rows: u8,
    text_size: TextSize,
    screen: Screen,
) -> Bounds {
    let scale = screen.height / 1080.0;
    let text = text_size.factor();
    let (width, height) = match kind {
        Kind::RelicReward => (1000.0 * scale * text, 295.0 * text),
        Kind::RelicRecommendation => (480.0 * text, (60.0 + 30.0 * f64::from(rows)) * text),
        Kind::Riven => (700.0 * text, 900.0 * scale * text),
        Kind::Ducatering => (420.0 * text, 460.0 * text),
        Kind::Notification => (380.0 * text, 100.0 * text),
    };
    let width = width.round();
    let height = height.min(screen.height - 2.0 * MARGIN).round();
    let middle = (screen.width / 2.0 - width / 2.0).round();
    let right = (screen.width - width - MARGIN).round();
    let bottom = (screen.height - height - MARGIN).round();
    let (x, y) = match placement {
        OverlayPlacement::TopLeft => (MARGIN, MARGIN),
        OverlayPlacement::TopCentre => (middle, MARGIN),
        OverlayPlacement::TopRight => (right, MARGIN),
        OverlayPlacement::BottomLeft => (MARGIN, bottom),
        OverlayPlacement::BottomCentre => (middle, bottom),
        OverlayPlacement::BottomRight => (right, bottom),
        OverlayPlacement::Centre => match kind {
            Kind::RelicReward => (middle - 15.0, (630.0 * scale).round()),
            Kind::RelicRecommendation | Kind::Riven | Kind::Ducatering | Kind::Notification => {
                (middle, (screen.height / 2.0 - height / 2.0).round())
            }
        },
    };
    Bounds {
        width,
        height,
        x: x + screen.x,
        y: y + screen.y,
    }
}

pub(super) fn place<R: Runtime>(window: &WebviewWindow<R>, bounds: Bounds, scale: f64) {
    let placed = window
        .set_size(LogicalSize::new(bounds.width, bounds.height))
        .and_then(|()| window.set_position(LogicalPosition::new(bounds.x, bounds.y)))
        .and_then(|()| window.set_zoom(1.0 / scale));
    if let Err(error) = placed {
        warn!(label = window.label(), %error, "Overlay window resize failed");
    }
}

pub(super) fn park<R: Runtime>(app: &AppHandle<R>, kind: Kind) {
    let Some(window) = app.get_webview_window(kind.label()) else {
        return;
    };
    #[cfg(windows)]
    if let Err(error) = no_activate::hide(&window) {
        warn!(label = kind.label(), %error, "Overlay hide failed");
    }
    #[cfg(not(windows))]
    if let Err(error) = window.hide() {
        warn!(label = kind.label(), %error, "Overlay hide failed");
    }
}

pub(super) fn retire<R: Runtime>(app: &AppHandle<R>, kind: Kind) {
    let Some(window) = app.get_webview_window(kind.label()) else {
        return;
    };
    if let Err(error) = window.close() {
        warn!(label = kind.label(), %error, "Overlay window close failed");
    }
}

pub(super) fn reveal<R: Runtime>(app: &AppHandle<R>, kind: Kind) {
    let Some(window) = app.get_webview_window(kind.label()) else {
        debug!(label = kind.label(), "No overlay window to show");
        return;
    };
    #[cfg(windows)]
    if let Err(error) = no_activate::show_on_top(&window) {
        warn!(label = kind.label(), %error, "Overlay show failed");
    }
    #[cfg(not(windows))]
    if let Err(error) = window.show() {
        warn!(label = kind.label(), %error, "Overlay show failed");
        return;
    }
    #[cfg(not(windows))]
    if let Err(error) = window.set_always_on_top(true) {
        warn!(
            label = kind.label(),
            %error,
            "Overlay always on top failed",
        );
    }
}

/// Showing and hiding overlays on Windows without ever activating them.
///
/// tao's `show()` is `ShowWindow(SW_SHOW)`, which activates the window and takes focus from
/// the game: it only shows without activating while a window is being created, and
/// `focusable(false)` doesn't stop it. So overlays are shown and hidden here instead, and tao
/// is left believing they stay hidden. tao re-applies `SW_HIDE` to a window it thinks hidden
/// whenever one of its flags changes, so overlays must not change flags once open; at worst
/// that hides one until it is next shown, never focuses it.
#[cfg(windows)]
mod no_activate {
    use std::ffi::c_void;

    use tauri::{Runtime, WebviewWindow};

    const SW_HIDE: i32 = 0;
    const SW_SHOWNOACTIVATE: i32 = 4;
    const HWND_TOPMOST: isize = -1;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOACTIVATE: u32 = 0x0010;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn ShowWindow(window: *mut c_void, command: i32) -> i32;
        fn SetWindowPos(
            window: *mut c_void,
            after: *mut c_void,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            flags: u32,
        ) -> i32;
    }

    /// Runs `apply` on the window's handle on the UI thread, which owns it.
    fn with_handle<R: Runtime>(
        window: &WebviewWindow<R>,
        apply: impl FnOnce(*mut c_void) + Send + 'static,
    ) -> tauri::Result<()> {
        // Handles are plain numbers that stay valid while the window lives; only the pointer
        // type keeps them from crossing threads.
        let handle = window.hwnd()?.0 as isize;
        window.run_on_main_thread(move || apply(handle as *mut c_void))
    }

    pub(super) fn show_on_top<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
        with_handle(window, |handle| {
            // SAFETY: `handle` is this overlay's live window handle; neither call dereferences
            // memory of ours, and both only ask the window manager to show and raise it
            // without activation.
            unsafe {
                ShowWindow(handle, SW_SHOWNOACTIVATE);
                SetWindowPos(
                    handle,
                    HWND_TOPMOST as *mut c_void,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        })
    }

    pub(super) fn hide<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
        with_handle(window, |handle| {
            // SAFETY: as in `show_on_top`; hiding never activates anything.
            unsafe {
                ShowWindow(handle, SW_HIDE);
            }
        })
    }
}

pub(super) fn screen_of<R: Runtime>(app: &AppHandle<R>, game: Option<MonitorRect>) -> Screen {
    let primary = app.primary_monitor().ok().flatten();
    let Some(rect) = game else {
        return primary
            .as_ref()
            .map_or(DEFAULT_SCREEN, Screen::from_monitor);
    };
    let primary_origin = primary
        .as_ref()
        .map_or(PhysicalPosition::new(0, 0), |monitor| *monitor.position());
    let screen = Screen::from_game(rect, primary_origin, 1.0);
    let scale = app
        .monitor_from_point(
            screen.x + screen.width / 2.0,
            screen.y + screen.height / 2.0,
        )
        .ok()
        .flatten()
        .or(primary)
        .map_or(1.0, |monitor| monitor.scale_factor());
    Screen { scale, ..screen }
}

pub(super) fn open<R: Runtime>(app: &AppHandle<R>, kind: Kind, bounds: Bounds, scale: f64) {
    let built = WebviewWindowBuilder::new(
        app,
        kind.label(),
        WebviewUrl::App(PathBuf::from(kind.route())),
    )
    .title(kind.title())
    .inner_size(bounds.width, bounds.height)
    .position(bounds.x, bounds.y)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .focusable(false)
    // Also keeps the webview from moving focus into itself when it is created.
    .focused(false)
    .resizable(cfg!(target_os = "linux"))
    .shadow(false)
    .visible(false)
    .build();
    match built {
        Ok(window) => {
            if let Err(error) = window.set_zoom(1.0 / scale) {
                warn!(
                    label = kind.label(),
                    scale,
                    %error,
                    "Webview zoom to game pixels failed",
                );
            }

            #[cfg(target_os = "linux")]
            bypass_window_manager(&window, kind);
            #[cfg(not(target_os = "linux"))]
            click_through(&window, kind);
        }
        Err(error) => warn!(label = kind.label(), %error, "Overlay window build failed"),
    }
}

fn click_through<R: Runtime>(window: &WebviewWindow<R>, kind: Kind) {
    if let Err(error) = window.set_ignore_cursor_events(true) {
        warn!(
            label = kind.label(),
            %error,
            "Overlay click through failed"
        );
    }
}

#[cfg(target_os = "linux")]
fn bypass_window_manager<R: Runtime>(window: &WebviewWindow<R>, kind: Kind) {
    let owned = window.clone();
    let queued = window.run_on_main_thread(move || {
        use gtk::prelude::WidgetExt;
        if let Ok(gtk_window) = owned.gtk_window() {
            gtk_window.realize();
            if let Some(surface) = gtk_window.window() {
                surface.set_override_redirect(true);
            }
        }

        click_through(&owned, kind);
    });
    if let Err(error) = queued {
        warn!(
            label = kind.label(),
            %error,
            "Override redirect queue failed",
        );
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "bounds are rounded to whole pixels")]
mod tests {
    use super::*;
    use crate::settings::RECOMMENDATION_COUNT_DEFAULT;

    const FULL_HD: Screen = Screen {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
        scale: 1.0,
    };

    #[test]
    fn text_size_grows_the_window_in_place() {
        let medium = bounds_for(
            Kind::Ducatering,
            OverlayPlacement::BottomRight,
            RECOMMENDATION_COUNT_DEFAULT,
            TextSize::Medium,
            FULL_HD,
        );
        assert_eq!(
            medium,
            Bounds {
                width: 473.0,
                height: 518.0,
                x: 1427.0,
                y: 542.0,
            }
        );

        let large = bounds_for(
            Kind::Riven,
            OverlayPlacement::Centre,
            RECOMMENDATION_COUNT_DEFAULT,
            TextSize::Large,
            FULL_HD,
        );
        assert_eq!(large.width, 875.0);
        assert_eq!(large.height, FULL_HD.height - 2.0 * MARGIN);
    }

    #[test]
    fn game_pixels_at_every_scale() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let screen = Screen { scale, ..FULL_HD };
            let physical = bounds_for(
                Kind::RelicRecommendation,
                OverlayPlacement::TopRight,
                RECOMMENDATION_COUNT_DEFAULT,
                TextSize::Small,
                screen,
            );
            assert_eq!(
                physical,
                Bounds {
                    width: 480.0,
                    height: 240.0,
                    x: 1420.0,
                    y: 20.0,
                }
            );
            let window = super::logical(physical, scale);
            assert_eq!(window.width * scale, 480.0);
            assert_eq!(window.x * scale, 1420.0);
        }
        assert_eq!(
            super::logical(
                bounds_for(
                    Kind::RelicRecommendation,
                    OverlayPlacement::TopRight,
                    RECOMMENDATION_COUNT_DEFAULT,
                    TextSize::Small,
                    Screen {
                        scale: 1.25,
                        ..FULL_HD
                    },
                ),
                1.25
            ),
            Bounds {
                width: 384.0,
                height: 192.0,
                x: 1136.0,
                y: 16.0,
            }
        );

        let compositor_upscales_xwayland = Screen {
            x: 0.0,
            y: 0.0,
            width: 2560.0,
            height: 1440.0,
            scale: 1.0,
        };
        let physical = bounds_for(
            Kind::RelicRecommendation,
            OverlayPlacement::TopRight,
            RECOMMENDATION_COUNT_DEFAULT,
            TextSize::Small,
            compositor_upscales_xwayland,
        );
        assert_eq!(
            physical,
            Bounds {
                width: 480.0,
                height: 240.0,
                x: 2060.0,
                y: 20.0,
            }
        );
        assert_eq!(super::logical(physical, 1.0), physical);

        let xwayland_carries_the_scaling_factor = Screen {
            x: 0.0,
            y: 0.0,
            width: 3840.0,
            height: 2160.0,
            scale: 2.0,
        };
        let physical = bounds_for(
            Kind::RelicRecommendation,
            OverlayPlacement::TopRight,
            RECOMMENDATION_COUNT_DEFAULT,
            TextSize::Small,
            xwayland_carries_the_scaling_factor,
        );
        assert_eq!(
            physical,
            Bounds {
                width: 480.0,
                height: 240.0,
                x: 3340.0,
                y: 20.0,
            }
        );
        assert_eq!(
            super::logical(physical, 2.0),
            Bounds {
                width: 240.0,
                height: 120.0,
                x: 1670.0,
                y: 10.0,
            }
        );
    }

    #[test]
    fn reward_band_per_resolution() {
        let full = bounds_for(
            Kind::RelicReward,
            OverlayPlacement::Centre,
            RECOMMENDATION_COUNT_DEFAULT,
            TextSize::Small,
            FULL_HD,
        );
        assert_eq!(
            full,
            Bounds {
                width: 1000.0,
                height: 295.0,
                x: 445.0,
                y: 630.0,
            }
        );

        let tall = bounds_for(
            Kind::RelicReward,
            OverlayPlacement::Centre,
            RECOMMENDATION_COUNT_DEFAULT,
            TextSize::Small,
            Screen {
                x: 0.0,
                y: 0.0,
                width: 2560.0,
                height: 1440.0,
                scale: 1.0,
            },
        );
        assert_eq!(tall.width, 1333.0);
        assert_eq!(tall.y, 840.0);
        assert_eq!(tall.x, 599.0);
    }

    #[test]
    fn notification_positions() {
        let at = |position: &str| {
            let settings: Settings =
                serde_json::from_str(&format!(r#"{{"toasts_in_game_position":"{position}"}}"#))
                    .unwrap();
            let bounds = bounds_for(
                Kind::Notification,
                placement_of(&settings, Kind::Notification),
                RECOMMENDATION_COUNT_DEFAULT,
                TextSize::Small,
                FULL_HD,
            );
            assert_eq!((bounds.width, bounds.height), (380.0, 100.0));
            (bounds.x, bounds.y)
        };
        assert_eq!(at("top_left"), (20.0, 20.0));
        assert_eq!(at("top_centre"), (770.0, 20.0));
        assert_eq!(at("top_right"), (1520.0, 20.0));
        assert_eq!(at("bottom_left"), (20.0, 960.0));
        assert_eq!(at("bottom_centre"), (770.0, 960.0));
        assert_eq!(at("bottom_right"), (1520.0, 960.0));
    }

    #[test]
    fn shipped_placements() {
        let settings = Settings::default();
        assert_eq!(
            placement_of(&settings, Kind::RelicReward),
            OverlayPlacement::Centre
        );
        assert_eq!(
            placement_of(&settings, Kind::RelicRecommendation),
            OverlayPlacement::TopRight
        );
        assert_eq!(
            placement_of(&settings, Kind::Riven),
            OverlayPlacement::TopLeft
        );
        assert_eq!(
            bounds_for(
                Kind::RelicRecommendation,
                OverlayPlacement::TopRight,
                RECOMMENDATION_COUNT_DEFAULT,
                TextSize::Small,
                FULL_HD
            ),
            Bounds {
                width: 480.0,
                height: 240.0,
                x: 1420.0,
                y: 20.0,
            }
        );
        assert_eq!(
            bounds_for(
                Kind::Riven,
                OverlayPlacement::TopLeft,
                RECOMMENDATION_COUNT_DEFAULT,
                TextSize::Small,
                FULL_HD
            ),
            Bounds {
                width: 700.0,
                height: 900.0,
                x: 20.0,
                y: 20.0,
            }
        );
        assert_eq!(
            bounds_for(
                Kind::Riven,
                OverlayPlacement::TopLeft,
                RECOMMENDATION_COUNT_DEFAULT,
                TextSize::Small,
                Screen {
                    x: 0.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 800.0,
                    scale: 1.0,
                }
            )
            .height,
            667.0
        );
    }

    #[test]
    fn placement_corners() {
        let corner = |placement| {
            let bounds = bounds_for(
                Kind::RelicRecommendation,
                placement,
                RECOMMENDATION_COUNT_DEFAULT,
                TextSize::Small,
                FULL_HD,
            );
            assert_eq!(bounds.width, 480.0);
            assert_eq!(bounds.height, 240.0);
            (bounds.x, bounds.y)
        };
        assert_eq!(corner(OverlayPlacement::TopLeft), (20.0, 20.0));
        assert_eq!(corner(OverlayPlacement::TopRight), (1420.0, 20.0));
        assert_eq!(corner(OverlayPlacement::BottomLeft), (20.0, 820.0));
        assert_eq!(corner(OverlayPlacement::BottomRight), (1420.0, 820.0));
        assert_eq!(corner(OverlayPlacement::Centre), (720.0, 420.0));

        let riven = |placement| {
            let bounds = bounds_for(
                Kind::Riven,
                placement,
                RECOMMENDATION_COUNT_DEFAULT,
                TextSize::Small,
                FULL_HD,
            );
            assert_eq!(bounds.width, 700.0);
            assert_eq!(bounds.height, 900.0);
            (bounds.x, bounds.y)
        };
        assert_eq!(riven(OverlayPlacement::TopLeft), (20.0, 20.0));
        assert_eq!(riven(OverlayPlacement::TopRight), (1200.0, 20.0));
        assert_eq!(riven(OverlayPlacement::BottomLeft), (20.0, 160.0));
        assert_eq!(riven(OverlayPlacement::BottomRight), (1200.0, 160.0));
        assert_eq!(riven(OverlayPlacement::Centre), (610.0, 90.0));

        let reward = |placement| {
            let bounds = bounds_for(
                Kind::RelicReward,
                placement,
                RECOMMENDATION_COUNT_DEFAULT,
                TextSize::Small,
                FULL_HD,
            );
            assert_eq!(bounds.width, 1000.0);
            assert_eq!(bounds.height, 295.0);
            (bounds.x, bounds.y)
        };
        assert_eq!(reward(OverlayPlacement::TopLeft), (20.0, 20.0));
        assert_eq!(reward(OverlayPlacement::TopRight), (900.0, 20.0));
        assert_eq!(reward(OverlayPlacement::BottomLeft), (20.0, 765.0));
        assert_eq!(reward(OverlayPlacement::BottomRight), (900.0, 765.0));
        assert_eq!(reward(OverlayPlacement::Centre), (445.0, 630.0));
    }

    #[test]
    fn larger_screen() {
        let screen = Screen {
            x: 0.0,
            y: 0.0,
            width: 2560.0,
            height: 1440.0,
            scale: 1.0,
        };
        assert_eq!(
            bounds_for(
                Kind::RelicRecommendation,
                OverlayPlacement::BottomRight,
                RECOMMENDATION_COUNT_DEFAULT,
                TextSize::Small,
                screen
            ),
            Bounds {
                width: 480.0,
                height: 240.0,
                x: 2060.0,
                y: 1180.0,
            }
        );
        assert_eq!(
            bounds_for(
                Kind::Riven,
                OverlayPlacement::TopRight,
                RECOMMENDATION_COUNT_DEFAULT,
                TextSize::Small,
                screen
            ),
            Bounds {
                width: 700.0,
                height: 1200.0,
                x: 1840.0,
                y: 20.0,
            }
        );
    }

    #[test]
    fn game_monitor_origin_moves_the_window() {
        let second_monitor = Screen {
            x: 1920.0,
            y: -360.0,
            width: 2560.0,
            height: 1440.0,
            scale: 1.0,
        };
        let top_left = bounds_for(
            Kind::Riven,
            OverlayPlacement::TopLeft,
            RECOMMENDATION_COUNT_DEFAULT,
            TextSize::Small,
            second_monitor,
        );
        assert_eq!((top_left.x, top_left.y), (1940.0, -340.0));
        let bottom_right = bounds_for(
            Kind::Riven,
            OverlayPlacement::BottomRight,
            RECOMMENDATION_COUNT_DEFAULT,
            TextSize::Small,
            second_monitor,
        );
        assert_eq!((bottom_right.width, bottom_right.height), (700.0, 1200.0));
        assert_eq!((bottom_right.x, bottom_right.y), (3760.0, -140.0));
        let centre = bounds_for(
            Kind::RelicReward,
            OverlayPlacement::Centre,
            RECOMMENDATION_COUNT_DEFAULT,
            TextSize::Small,
            second_monitor,
        );
        assert_eq!((centre.x, centre.y), (2519.0, 480.0));
        let scaled = super::logical(top_left, 2.0);
        assert_eq!((scaled.x, scaled.y), (970.0, -170.0));
    }

    #[test]
    fn game_rectangle_becomes_the_screen() {
        let rect = MonitorRect {
            left: 1920,
            top: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(
            Screen::from_game(rect, PhysicalPosition::new(0, 0), 1.25),
            Screen {
                x: 1920.0,
                y: 0.0,
                scale: 1.25,
                ..FULL_HD
            }
        );
    }

    #[test]
    fn game_rectangle_offset_by_primary() {
        let middle_of_three = MonitorRect {
            left: 0,
            top: 0,
            width: 2560,
            height: 1440,
        };
        let primary = PhysicalPosition::new(2560, -360);
        assert_eq!(
            Screen::from_game(middle_of_three, primary, 1.0),
            Screen {
                x: 2560.0,
                y: -360.0,
                width: 2560.0,
                height: 1440.0,
                scale: 1.0,
            }
        );

        let left_of_primary = MonitorRect {
            left: -2560,
            top: 0,
            width: 2560,
            height: 1440,
        };
        assert_eq!(Screen::from_game(left_of_primary, primary, 1.0).x, 0.0);
    }
}
