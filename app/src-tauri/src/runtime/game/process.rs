use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Runtime};
use tracing::{debug, warn};
use wf_mem::{GAME_PROCESS, MemError};

use crate::runtime::{blocking, emit_status};
use crate::state::{AppState, write};

const PROCESS_POLL: Duration = Duration::from_secs(5);

pub(crate) async fn process_task<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) -> ! {
    loop {
        let found = match blocking("find game process", || wf_mem::find_process(GAME_PROCESS)).await
        {
            Some(Ok(pid)) => Some(pid),
            Some(Err(MemError::ProcessNotFound(_))) | None => None,
            Some(Err(error)) => {
                warn!(
                    %error,
                    "Process list was unreadable, treating the game as not running",
                );
                None
            }
        };
        let changed = {
            let mut status = write(&state.status);
            let changed = status.game_detected != found.is_some() || status.pid != found;
            status.game_detected = found.is_some();
            status.pid = found;
            changed
        };
        if changed {
            debug!(
                pid = found,
                detected = found.is_some(),
                "Game process changed"
            );
            emit_status(&app, &state);
            if found.is_some() {
                state.rescan.notify_one();
            } else {
                state.focus.forget();
            }
        }
        tokio::time::sleep(PROCESS_POLL).await;
    }
}
