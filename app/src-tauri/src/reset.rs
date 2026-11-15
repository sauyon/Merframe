use std::path::{Path, PathBuf};

use anyhow::Context;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Runtime};

use crate::settings::{self, unless_missing};
use crate::state;

const PENDING_MARKER: &str = ".reset-pending";

pub struct FailedReset(pub String);

fn identifier_dirs<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<Vec<PathBuf>> {
    let paths = app.path();
    Ok(vec![
        paths.app_config_dir()?,
        paths.app_data_dir()?,
        paths.app_local_data_dir()?,
        paths.app_cache_dir()?,
        paths.app_log_dir()?,
    ])
}

pub fn request<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    settings::set_token(app, None)?;
    std::fs::write(state::data_dir(app)?.join(PENDING_MARKER), [])
        .context("Scheduling the Merframe reset")?;
    app.request_restart();
    Ok(())
}

fn remove(path: &Path) -> anyhow::Result<()> {
    let found = unless_missing(std::fs::symlink_metadata(path))
        .with_context(|| format!("Inspecting {}", path.display()))?;
    let Some(metadata) = found else {
        return Ok(());
    };

    let removed = if metadata.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    removed.with_context(|| format!("Deleting {}", path.display()))
}

fn clear_then_unmark(data: &Path, identifier_dirs: &[PathBuf]) -> anyhow::Result<()> {
    for path in identifier_dirs {
        remove(path)?;
    }

    for entry in std::fs::read_dir(data).context("Reading the Merframe data folder")? {
        let entry = entry?;
        if entry.file_name() != PENDING_MARKER {
            remove(&entry.path())?;
        }
    }

    remove(&data.join(PENDING_MARKER))
}

fn finish_pending<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    let data = state::data_dir(app)?;
    if !data.join(PENDING_MARKER).try_exists()? {
        return Ok(());
    }

    settings::set_token(app, None)?;
    clear_then_unmark(&data, &identifier_dirs(app)?)
}

pub fn first_plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("reset")
        .setup(|app, _| {
            if let Err(error) = finish_pending(app) {
                app.manage(FailedReset(format!(
                    "Merframe could not finish resetting. Close Merframe, unlock your keyring and check file permissions, then reopen it to retry. The cause: {error:#}"
                )));
            }

            Ok(())
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("merframe-reset-{name}-{}", std::process::id()))
    }

    #[test]
    fn reset_clears_everything() {
        let root = scratch_dir("all");
        let data = root.join("Merframe");
        let webview = root.join("webview");
        std::fs::create_dir_all(data.join("images")).unwrap();
        std::fs::create_dir_all(&webview).unwrap();
        for path in [
            data.join(PENDING_MARKER),
            data.join("merframe.sqlite"),
            data.join("images/item.png"),
            webview.join("preferences"),
        ] {
            std::fs::write(path, "saved data").unwrap();
        }

        clear_then_unmark(&data, &[webview.clone(), webview.clone()]).unwrap();
        assert_eq!(std::fs::read_dir(&data).unwrap().count(), 0);
        assert!(!webview.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn failed_reset_keeps_marker() {
        let root = scratch_dir("failed");
        let data = root.join("Merframe");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join(PENDING_MARKER), []).unwrap();
        std::fs::write(data.join("merframe.sqlite"), "history").unwrap();
        let blocker = root.join("file");
        std::fs::write(&blocker, []).unwrap();

        assert!(clear_then_unmark(&data, &[blocker.join("child")]).is_err());
        assert!(data.join(PENDING_MARKER).exists());
        assert!(data.join("merframe.sqlite").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_target_survives() {
        let root = scratch_dir("link");
        let data = root.join("Merframe");
        let outside = root.join("outside");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(data.join(PENDING_MARKER), []).unwrap();
        std::fs::write(outside.join("keep"), "unrelated").unwrap();
        std::os::unix::fs::symlink(&outside, data.join("link")).unwrap();

        clear_then_unmark(&data, &[]).unwrap();
        assert!(outside.join("keep").exists());
        assert!(!data.join("link").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
