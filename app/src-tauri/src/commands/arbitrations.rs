use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use chrono::{Local, Utc};
use serde::Deserialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder};
use tracing::info;
use wf_core::{ArbitrationRun, svg_report, text_report};

use super::{Shared, ask_path, game_log_path, pick_log, ready};
use crate::error::{CommandError, CommandResult};
use crate::runtime::{self, AppEvent};
use crate::state::{AppState, lock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ArbitrationRunExportFormat {
    Json,
    Text,
    Svg,
}

impl ArbitrationRunExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Text => "txt",
            Self::Svg => "svg",
        }
    }
}

fn export_body(run: &ArbitrationRun, format: ArbitrationRunExportFormat) -> CommandResult<String> {
    Ok(match format {
        ArbitrationRunExportFormat::Json => {
            serde_json::to_string_pretty(run).context("Serialising the run")?
        }
        ArbitrationRunExportFormat::Text => text_report(run),
        ArbitrationRunExportFormat::Svg => svg_report(run),
    })
}

const RUN_GONE: &str = "This run is no longer stored";

fn stored_run(core: &wf_core::Core, id: &str) -> CommandResult<ArbitrationRun> {
    core.arbitration_run(id)?
        .ok_or_else(|| CommandError::from(RUN_GONE))
}

#[tauri::command]
pub async fn export_arbitration_run<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    id: String,
    format: ArbitrationRunExportFormat,
) -> CommandResult<Option<String>> {
    let state = ready(&state).await?;
    let (body, file_name) = {
        let core = lock(&state.core);
        let run = stored_run(&core, &id)?;
        let body = export_body(&run, format)?;
        let node = run
            .node
            .clone()
            .unwrap_or_else(|| String::from("arbitration"));
        let stamp = run.started_at.map_or_else(String::new, |at| {
            at.with_timezone(&Local)
                .format("-%Y-%m-%d-%H%M")
                .to_string()
        });
        (body, format!("{node}{stamp}.{}", format.extension()))
    };

    let dialog = app
        .dialog()
        .file()
        .set_title("Save the arbitration run")
        .set_file_name(&file_name);
    let Some(path) = ask_path(dialog, FileDialogBuilder::blocking_save_file).await? else {
        return Ok(None);
    };

    tokio::fs::write(&path, body)
        .await
        .with_context(|| format!("Writing {}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
pub async fn arbitration_runs(state: Shared<'_>) -> CommandResult<Vec<ArbitrationRun>> {
    let state = ready(&state).await?;
    Ok(lock(&state.core).arbitration_runs()?)
}

#[tauri::command]
pub async fn arbitration_live_run(state: Shared<'_>) -> CommandResult<Option<ArbitrationRun>> {
    let state = ready(&state).await?;
    Ok(lock(&state.core).live_arbitration_run())
}

#[tauri::command]
pub async fn delete_arbitration_run<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    id: String,
) -> CommandResult<()> {
    let state = ready(&state).await?;
    lock(&state.core).delete_arbitration_run(&id)?;
    runtime::emit(&app, AppEvent::ArbitrationRunsUpdated);
    Ok(())
}

#[tauri::command]
pub async fn set_arbitration_run_vitus<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    id: String,
    vitus: Option<u32>,
) -> CommandResult<()> {
    let state = ready(&state).await?;
    let stored = lock(&state.core).set_arbitration_run_vitus(&id, vitus)?;
    if !stored {
        return Err(CommandError::from(RUN_GONE));
    }

    runtime::emit(&app, AppEvent::ArbitrationRunsUpdated);
    Ok(())
}

#[tauri::command]
pub async fn import_arbitration_log<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
) -> CommandResult<Option<usize>> {
    let state = ready(&state).await?;
    let Some(path) = pick_log(&app, "Choose an EE.log with arbitration runs").await? else {
        return Ok(None);
    };

    Ok(Some(import_log(&app, state, path).await?))
}

#[tauri::command]
pub async fn refresh_arbitration_runs<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
) -> CommandResult<usize> {
    let state = ready(&state).await?;
    let path = game_log_path(&state)?;
    import_log(&app, state, path).await
}

async fn import_log<R: Runtime>(
    app: &AppHandle<R>,
    state: Arc<AppState>,
    path: PathBuf,
) -> CommandResult<usize> {
    let imported = tauri::async_runtime::spawn_blocking(move || {
        let lines =
            wf_log::read_all(&path).with_context(|| format!("Reading {}", path.display()))?;
        let imported = lock(&state.core).import_arbitration_runs(&lines, Utc::now())?;
        info!(imported, path = %path.display(), "Arbitration runs imported from a log");
        anyhow::Ok(imported)
    })
    .await??;
    runtime::emit(app, AppEvent::ArbitrationRunsUpdated);
    Ok(imported)
}
