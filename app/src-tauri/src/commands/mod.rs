use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use chrono::{DateTime, Utc};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};
use wf_core::TimeRange;

use crate::error::{CommandError, CommandResult};
use crate::state::{AppState, AppStateCell, lock, read};

pub mod analytics;
pub mod app;
pub mod arbitrations;
pub mod market;
pub mod tabs;
pub mod trades;
pub mod world;

pub use world::WorldStateView;

type Shared<'a> = State<'a, AppStateCell>;

async fn ready(cell: &AppStateCell) -> CommandResult<Arc<AppState>> {
    cell.wait().await.clone().map_err(CommandError::from)
}

fn game_log_path(state: &AppState) -> CommandResult<PathBuf> {
    Ok(read(&state.settings)
        .log_path()
        .context("EE.log not found on this machine")?)
}

async fn ask_path<R: Runtime>(
    dialog: FileDialogBuilder<R>,
    ask: fn(FileDialogBuilder<R>) -> Option<FilePath>,
) -> CommandResult<Option<PathBuf>> {
    let picked = tauri::async_runtime::spawn_blocking(move || ask(dialog)).await?;
    Ok(picked
        .as_ref()
        .and_then(FilePath::as_path)
        .map(Path::to_path_buf))
}

async fn pick_log<R: Runtime>(app: &AppHandle<R>, title: &str) -> CommandResult<Option<PathBuf>> {
    let dialog = app
        .dialog()
        .file()
        .set_title(title)
        .add_filter("Warframe log", &["log", "txt"]);
    ask_path(dialog, FileDialogBuilder::blocking_pick_file).await
}

fn missing_inventory() -> CommandError {
    CommandError::from("No inventory has been read from the game yet")
}

fn since(since_ms: Option<i64>) -> TimeRange {
    match since_ms.and_then(DateTime::<Utc>::from_timestamp_millis) {
        Some(from) => TimeRange::since(from),
        None => TimeRange::all(),
    }
}

async fn compute<T, F>(state: Arc<AppState>, build: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(&wf_core::Core) -> Option<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || build(&lock(&state.core)))
        .await?
        .ok_or_else(missing_inventory)
}
