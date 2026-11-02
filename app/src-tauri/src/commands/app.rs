use std::sync::Arc;

use anyhow::Context;
use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_opener::OpenerExt;

use super::{Shared, ready};
use crate::error::{CommandError, CommandResult};
use crate::event_feed::EventFeed;
use crate::overlay;
use crate::runtime::{self, AppEvent};
use crate::settings::{self, Settings};
use crate::state::{GameStatus, LOG_FILE, lock, read};

#[tauri::command]
pub async fn game_status(state: Shared<'_>) -> CommandResult<GameStatus> {
    let state = ready(&state).await?;
    Ok(state.status_snapshot())
}

#[tauri::command]
pub fn updates_supported() -> bool {
    tauri::utils::platform::bundle_type().is_some()
}

#[tauri::command]
pub async fn overlay_state(state: Shared<'_>) -> CommandResult<overlay::OverlayState> {
    Ok(ready(&state).await?.overlays.snapshot())
}

#[tauri::command]
pub async fn overlay_page_ready<R: Runtime>(
    window: tauri::WebviewWindow<R>,
    state: Shared<'_>,
) -> CommandResult<()> {
    overlay::on_page_ready(window.app_handle(), &ready(&state).await?, window.label());
    Ok(())
}

#[tauri::command]
pub async fn overlay_notify<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    title: String,
    body: String,
) -> CommandResult<()> {
    overlay::notify(&app, &ready(&state).await?, title, body);
    Ok(())
}

#[tauri::command]
pub async fn overlay_test<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    kind: overlay::Kind,
) -> CommandResult<()> {
    overlay::toggle_preview(&app, &ready(&state).await?, kind)?;
    Ok(())
}

#[tauri::command]
pub async fn overlay_content_shrank<R: Runtime>(window: tauri::WebviewWindow<R>) {
    overlay::on_content_shrank(&window);
}

#[tauri::command]
pub async fn rescan_inventory<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
) -> CommandResult<GameStatus> {
    let state = ready(&state).await?;
    if runtime::acquire(&app, &state).await {
        Ok(state.status_snapshot())
    } else {
        let status = state.status_snapshot();
        Err(CommandError::from(
            status
                .last_scan_error
                .unwrap_or_else(|| "The inventory scan failed".to_owned()),
        ))
    }
}

#[tauri::command]
pub async fn export<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
) -> CommandResult<Option<String>> {
    let state = ready(&state).await?;
    let dialog = app.dialog().file().set_title("Export inventory JSON");
    let picked =
        tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_folder()).await?;
    let Some(dir) = picked.as_ref().and_then(FilePath::as_path) else {
        return Ok(None);
    };

    let target = dir.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || lock(&state.core).export(&target, Utc::now()))
        .await??;
    Ok(Some(dir.display().to_string()))
}

#[tauri::command]
pub async fn pick_log_file<R: Runtime>(app: AppHandle<R>) -> CommandResult<Option<String>> {
    let dialog = app
        .dialog()
        .file()
        .set_title("Choose EE.log")
        .add_filter("Warframe log", &["log"]);
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_file()).await?;
    Ok(picked
        .as_ref()
        .and_then(FilePath::as_path)
        .map(|path| path.display().to_string()))
}

#[tauri::command]
pub async fn settings_get(state: Shared<'_>) -> CommandResult<Settings> {
    let state = ready(&state).await?;
    Ok(read(&state.settings).clone())
}

#[tauri::command]
pub async fn settings_set<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    settings: Settings,
) -> CommandResult<Settings> {
    let state = ready(&state).await?;
    settings.event_feed.check()?;
    let previous = settings::apply(&app, &state, &settings)?;
    if previous.alerts != settings.alerts {
        runtime::emit(&app, AppEvent::WorldStateUpdated);
    }
    overlay::apply_from(&app, &state);
    app.state::<EventFeed>().configure(settings.event_feed);
    Ok(settings)
}

#[tauri::command]
pub async fn test_notifications<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    settings: Settings,
) -> CommandResult<()> {
    let state = ready(&state).await?;
    runtime::test_notifications(&app, &state, &settings).await?;
    Ok(())
}

#[tauri::command]
pub async fn open_url<R: Runtime>(app: AppHandle<R>, url: String) -> CommandResult<()> {
    let url = browser_url(&url)?;
    Ok(app
        .opener()
        .open_url(url.as_str(), None::<&str>)
        .context("Opening the URL")?)
}

fn browser_url(url: &str) -> CommandResult<tauri::Url> {
    let parsed = tauri::Url::parse(url)
        .map_err(|_| CommandError::from("Not a valid browser URL".to_owned()))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(CommandError::from(
            "Only HTTP and HTTPS browser URLs without credentials are allowed".to_owned(),
        ));
    }
    Ok(parsed)
}

#[tauri::command]
pub async fn app_log_file(state: Shared<'_>) -> CommandResult<String> {
    let state = ready(&state).await?;
    Ok(state.data_dir.join(LOG_FILE).display().to_string())
}

#[tauri::command]
pub async fn open_data_folder<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
) -> CommandResult<()> {
    let state = ready(&state).await?;
    Ok(app
        .opener()
        .reveal_item_in_dir(state.data_dir.join(LOG_FILE))
        .context("Opening the data folder")?)
}

#[tauri::command]
pub async fn open_game_log_folder<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
) -> CommandResult<()> {
    let state = ready(&state).await?;
    let log = read(&state.settings)
        .log_path()
        .context("EE.log not found on this machine")?;
    Ok(app
        .opener()
        .reveal_item_in_dir(log)
        .context("Opening the EE.log folder")?)
}

#[tauri::command]
pub async fn item_image(state: Shared<'_>, image_name: String) -> CommandResult<String> {
    let state = ready(&state).await?;
    state.images.get(&image_name).await
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct ImagePath {
    pub name: String,
    pub path: String,
}

#[tauri::command]
pub async fn prefetch_images(
    state: Shared<'_>,
    names: Vec<String>,
) -> CommandResult<Vec<ImagePath>> {
    let state = ready(&state).await?;
    let mut wanted = names;
    wanted.sort_unstable();
    wanted.dedup();
    let requests = wanted
        .into_iter()
        .map(|name| {
            let shared = Arc::clone(&state);
            async move {
                let path = shared.images.get(&name).await.ok()?;
                Some(ImagePath { name, path })
            }
        })
        .collect::<Vec<_>>();
    let held = futures_util::future::join_all(requests)
        .await
        .into_iter()
        .flatten()
        .collect();
    Ok(held)
}

#[cfg(test)]
mod tests {
    use super::browser_url;

    #[test]
    fn browser_links_allow_web_pages() {
        for url in [
            "https://wiki.warframe.com/w/Braton_Prime",
            "https://warframe.market/items/braton_prime_set",
            "http://example.com/",
        ] {
            assert!(browser_url(url).is_ok(), "{url}");
        }
    }

    #[test]
    fn browser_links_reject_files_and_protocol_handlers() {
        for url in [
            "file:///C:/Windows/System32/calc.exe",
            "C:\\Windows\\System32\\calc.exe",
            "\\\\server\\share\\program.exe",
            "javascript:alert(1)",
            "ms-settings:windowsupdate",
            "steam://run/123",
            "https://user:password@example.com/",
            "not a URL",
        ] {
            assert!(browser_url(url).is_err(), "{url}");
        }
    }
}
