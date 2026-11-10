use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Runtime};
use tracing::{debug, info, warn};
use wf_market::{IncomingEvent, MarketSocket, StatusSetPayload, UserStatus};

use super::MARKET_SIGNED_OUT_POLL;
use crate::market_session;
use crate::state::{AppState, USER_AGENT, read, write};

pub(crate) async fn market_presence_task<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    let mut failures: u32 = 0;
    loop {
        let token = match state.market().token() {
            Some(token) if state.market_slug().is_some() => token.to_owned(),
            _ => {
                tokio::time::sleep(MARKET_SIGNED_OUT_POLL).await;
                continue;
            }
        };

        match handshake(token).await {
            Handshake::Accepted(mut socket) => {
                failures = 0;
                if let Err(error) = presence_session(&app, &state, &mut socket).await {
                    warn!(%error, "Market socket dropped");
                }
                market_session::set_live(&app, &state, None);
            }
            Handshake::Rejected => {
                info!("Market socket rejected the stored token");
                market_session::clear_rejected(&app, &state);
            }
            Handshake::Closed => failures = failures.saturating_add(1),
        }

        tokio::time::sleep(socket_backoff(failures)).await;
    }
}

fn socket_backoff(failures: u32) -> Duration {
    Duration::from_secs(10)
        .saturating_mul(1 << failures.min(6))
        .min(Duration::from_mins(10))
}

enum Handshake {
    Accepted(Box<MarketSocket>),
    Rejected,
    Closed,
}

async fn handshake(token: String) -> Handshake {
    match tokio::time::timeout(Duration::from_secs(15), authenticated(token)).await {
        Ok(Ok(handshake)) => handshake,
        Ok(Err(error)) => {
            warn!(%error, "Market socket handshake failed");
            Handshake::Closed
        }
        Err(_) => {
            warn!("Market socket did not acknowledge the token in fifteen seconds");
            Handshake::Closed
        }
    }
}

async fn authenticated(token: String) -> wf_market::Result<Handshake> {
    let mut socket = MarketSocket::connect(USER_AGENT).await?;
    socket.authenticate(token).await?;

    loop {
        match socket.next_event().await? {
            Some(IncomingEvent::AuthOk) => {
                debug!("Market socket authenticated");
                return Ok(Handshake::Accepted(Box::new(socket)));
            }
            Some(IncomingEvent::AuthError) => return Ok(Handshake::Rejected),
            Some(_) => {}
            None => return Ok(Handshake::Closed),
        }
    }
}

async fn presence_session<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
    socket: &mut MarketSocket,
) -> wf_market::Result<()> {
    let mut reported: Option<UserStatus> = None;
    let mut last_frame = Instant::now();
    loop {
        let game_detected = read(&state.status).game_detected;
        let presence = *read(&state.market_presence);
        let wanted = presence.next_status(game_detected);
        if let Some(status) = wanted {
            if reported != wanted {
                socket
                    .set_status(StatusSetPayload {
                        status,
                        duration: None,
                    })
                    .await?;
                reported = wanted;
                info!(status = ?status, "Market socket status set");
            }

            write(&state.market_presence).status_applied(presence.pause);
        }

        tokio::select! {
            event = socket.next_event() => {
                last_frame = Instant::now();
                match event? {
                    Some(IncomingEvent::AuthError) => {
                        info!("Market socket rejected the session, signing out");
                        market_session::clear_rejected(app, state);
                        return Ok(());
                    }
                    Some(IncomingEvent::StatusSet { payload, .. }) => {
                        reported = Some(payload.status);
                        market_session::set_live(app, state, reported);
                    }
                    Some(_) => {}
                    None => return Ok(()),
                }
            },
            () = state.market_presence_wake.notified() => {}
            () = tokio::time::sleep(Duration::from_secs(15)) => {
                if last_frame.elapsed() >= Duration::from_mins(3) {
                    info!("Market socket silent for three minutes, reconnecting");
                    return Ok(());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_backoff_caps_at_ten_minutes() {
        let sequence: Vec<u64> = (0..9)
            .map(|failures| socket_backoff(failures).as_secs())
            .collect();
        assert_eq!(sequence, [10, 20, 40, 80, 160, 320, 600, 600, 600]);
        assert_eq!(socket_backoff(99), Duration::from_mins(10));
    }
}
