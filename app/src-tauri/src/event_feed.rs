use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::http::header::HOST;
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Response, Sse};
use axum::routing::get;
use chrono::{DateTime, Utc};
use futures_util::stream::{self, Stream, StreamExt};
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, watch};
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, info, warn};
use wf_core::{
    ArbitrationAlert, ArbitrationRun, CoreEvent, FissureInfo, InventorySummary, RewardScreen,
    Trade, WorldRewardAlert,
};

use crate::runtime::{AppEvent, MarketAutoClose, update_status};
use crate::settings::EventFeedSettings;
use crate::state::{AppState, EventFeedStatus, GameStatus, lock, read};

const BUFFERED_EVENTS: usize = 64;

impl From<&GameStatus> for FeedStatus {
    fn from(val: &GameStatus) -> Self {
        FeedStatus {
            game_detected: val.game_detected,
            scanning: val.scanning,
            log_attached: val.log_attached,
            last_sync_at: val.last_sync_at,
            world_state_at: val.world_state_at,
            trades_remaining: val.trades_remaining,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FeedStatus {
    pub game_detected: bool,
    pub scanning: bool,
    pub log_attached: bool,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub world_state_at: Option<DateTime<Utc>>,
    pub trades_remaining: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FeedInventory {
    pub mr: u32,
    pub plat: i64,
    pub credits: i64,
    pub endo: i64,
    pub ducats: i64,
    pub changes: usize,
}

impl From<&InventorySummary> for FeedInventory {
    fn from(summary: &InventorySummary) -> Self {
        Self {
            mr: summary.mr,
            plat: summary.plat,
            credits: summary.credits,
            endo: summary.endo,
            ducats: summary.ducats,
            changes: summary.changes,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum FeedEvent {
    Status(FeedStatus),
    InventoryUpdated(FeedInventory),
    RelicRewardScreen {
        relic: Option<String>,
        screen: RewardScreen,
    },
    TradeCompleted {
        at: DateTime<Utc>,
        trade: Trade,
    },
    FissureAlert {
        fissure: FissureInfo,
    },
    WorldRewardAlert {
        reward: WorldRewardAlert,
    },
    ArbitrationAlert {
        alert: ArbitrationAlert,
    },
    ArbitrationRunUpdated {
        run: ArbitrationRun,
    },
    ArbitrationRunRecorded {
        run: ArbitrationRun,
    },
    TimerAlert {
        name: String,
        next_state: String,
        ends_at: DateTime<Utc>,
        remaining_secs: i64,
    },
    TradesUpdated,
    WorldStateUpdated,
    RivenDataUpdated,
    PricesUpdated,
    FavouriteUpdated,
    MarketAutoClosed(MarketAutoClose),
}

impl FeedEvent {
    pub fn forwarded(event: &AppEvent) -> Option<Self> {
        match event {
            AppEvent::StatusUpdated(status) => Some(Self::Status(status.into())),
            AppEvent::CoreEvent(envelope) => Self::from_core(&envelope.event),
            AppEvent::TradesUpdated => Some(Self::TradesUpdated),
            AppEvent::WorldStateUpdated => Some(Self::WorldStateUpdated),
            AppEvent::RivenDataUpdated => Some(Self::RivenDataUpdated),
            AppEvent::PricesUpdated => Some(Self::PricesUpdated),
            AppEvent::FavouriteUpdated => Some(Self::FavouriteUpdated),
            AppEvent::MarketAutoClosed(closed) => Some(Self::MarketAutoClosed(closed.clone())),
            AppEvent::InventoryUpdated(_)
            | AppEvent::SettingsUpdated(_)
            | AppEvent::ArbitrationRunUpdated
            | AppEvent::ArbitrationRunsUpdated
            | AppEvent::MarketUpdated(_)
            | AppEvent::MarketSessionRejected
            | AppEvent::MarketPresence(_)
            | AppEvent::OverlayState(_) => None,
        }
    }

    pub fn from_core(event: &CoreEvent) -> Option<Self> {
        match event {
            CoreEvent::InventoryUpdated(summary) => Some(Self::InventoryUpdated(summary.into())),
            CoreEvent::RelicRewardScreen { relic, screen } => Some(Self::RelicRewardScreen {
                relic: relic.clone(),
                screen: screen.clone(),
            }),
            CoreEvent::TradeCompleted { at, trade, .. } => Some(Self::TradeCompleted {
                at: *at,
                trade: trade.clone(),
            }),
            CoreEvent::FissureAlert { fissure } => Some(Self::FissureAlert {
                fissure: fissure.clone(),
            }),
            CoreEvent::WorldRewardAlert { reward } => Some(Self::WorldRewardAlert {
                reward: reward.clone(),
            }),
            CoreEvent::ArbitrationAlert { alert } => Some(Self::ArbitrationAlert {
                alert: alert.clone(),
            }),
            CoreEvent::ArbitrationRunUpdated { run } => Some(Self::ArbitrationRunUpdated {
                run: (**run).clone(),
            }),
            CoreEvent::ArbitrationRunRecorded { run } => Some(Self::ArbitrationRunRecorded {
                run: (**run).clone(),
            }),
            CoreEvent::TimerAlert {
                name,
                next_state,
                ends_at,
                remaining_secs,
            } => Some(Self::TimerAlert {
                name: name.clone(),
                next_state: next_state.clone(),
                ends_at: *ends_at,
                remaining_secs: *remaining_secs,
            }),
            CoreEvent::NewConversation { .. } => None,
        }
    }
}

/// Cloning clones [Arc]s. So cloning shares internal state and is cheap.
#[derive(Clone)]
pub struct EventFeed {
    events: broadcast::Sender<FeedEvent>,
    wanted: watch::Sender<Option<u16>>,
    last_status: Arc<Mutex<Option<FeedStatus>>>,
}

impl Default for EventFeed {
    fn default() -> Self {
        Self {
            events: broadcast::channel(BUFFERED_EVENTS).0,
            wanted: watch::Sender::new(None),
            last_status: Arc::default(),
        }
    }
}

impl EventFeed {
    pub fn publish(&self, event: &AppEvent) {
        let Some(forwarded) = FeedEvent::forwarded(event) else {
            return;
        };
        if let FeedEvent::Status(status) = &forwarded {
            let mut last = lock(&self.last_status);
            if last.as_ref() == Some(status) {
                return;
            }
            *last = Some(status.clone());
        }

        let _ = self.events.send(forwarded);
    }

    pub fn configure(&self, settings: EventFeedSettings) {
        let port = settings.port();
        self.wanted
            .send_if_modified(|wanted| std::mem::replace(wanted, port) != port);
    }
}

pub async fn serve<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    let feed = app.state::<EventFeed>();
    feed.configure(read(&state.settings).event_feed);
    let mut wanted = feed.wanted.subscribe();

    loop {
        let Some(port) = *wanted.borrow_and_update() else {
            if read(&state.status).event_feed != EventFeedStatus::Off {
                report(&app, &state, EventFeedStatus::Off);
            }

            let _ = wanted.changed().await;
            continue;
        };

        let listener = match TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
            Ok(listener) => listener,
            Err(error) => {
                let cause = error.to_string();
                warn!(port, cause, "Event feed cannot listen");
                report(&app, &state, EventFeedStatus::Failed { port, cause });
                let _ = wanted.changed().await;
                continue;
            }
        };

        info!(port, "Event feed listening");
        report(&app, &state, EventFeedStatus::Listening { port });

        let mut stop = wanted.clone();
        let served = axum::serve(listener, router(feed.inner().clone(), port))
            .with_graceful_shutdown(async move {
                let _ = stop.changed().await;
            })
            .await;
        if let Err(error) = served {
            warn!(%error, "Event feed stopped with an error");
        }

        info!(port, "Event feed stopped");
    }
}

fn router(feed: EventFeed, port: u16) -> Router {
    Router::new()
        .route("/events", get(subscribe))
        .layer(middleware::from_fn(move |request, next| {
            local_only(port, request, next)
        }))
        .with_state(feed)
}

fn report<R: Runtime>(app: &AppHandle<R>, state: &AppState, status: EventFeedStatus) {
    update_status(app, state, |game| game.event_feed = status);
}

async fn local_only(port: u16, request: Request, next: Next) -> Response {
    let local = request
        .headers()
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|host| host.rsplit_once(':'))
        .is_some_and(|(name, host_port)| {
            matches!(name, "127.0.0.1" | "localhost") && host_port.parse() == Ok(port)
        });
    if !local {
        return StatusCode::FORBIDDEN.into_response();
    }

    next.run(request).await
}

async fn subscribe(
    State(feed): State<EventFeed>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let updates = BroadcastStream::new(feed.events.subscribe()).filter_map(|received| async move {
        received
            .inspect_err(|lagged| debug!(%lagged, "Event feed client fell behind"))
            .ok()
    });
    let current = lock(&feed.last_status).clone().map(FeedEvent::Status);
    let mut stop = feed.wanted.subscribe();
    let events = stream::iter(current)
        .chain(updates)
        .take_until(async move {
            let _ = stop.changed().await;
        })
        .map(|event| Event::default().json_data(&event));

    Sse::new(events).keep_alive(KeepAlive::default())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::{Value, json};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpStream;
    use wf_core::{InventorySummary, Trade, TradeItem};

    use super::*;
    use crate::envelope::CoreEventEnvelope;
    use crate::market::MarketSnapshot;
    use crate::settings::MarketAccount;

    fn forwarded_json(event: &AppEvent) -> Option<Value> {
        FeedEvent::forwarded(event).map(|forwarded| serde_json::to_value(forwarded).unwrap())
    }

    async fn served(feed: &EventFeed) -> u16 {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        feed.configure(EventFeedSettings {
            event_feed_enabled: true,
            event_feed_port: port,
        });
        tokio::spawn(axum::serve(listener, router(feed.clone(), port)).into_future());

        port
    }

    async fn get_events(port: u16, host: &str) -> (String, BufReader<TcpStream>) {
        let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        let request =
            format!("GET /events HTTP/1.1\r\nHost: {host}\r\nAccept: text/event-stream\r\n\r\n");
        client.write_all(request.as_bytes()).await.unwrap();
        let mut reader = BufReader::new(client);
        let mut status_line = String::new();
        reader.read_line(&mut status_line).await.unwrap();
        let mut header = String::new();
        while header != "\r\n" {
            header.clear();
            reader.read_line(&mut header).await.unwrap();
        }

        (status_line, reader)
    }

    async fn next_event(reader: &mut BufReader<TcpStream>) -> Option<Value> {
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            if let Some(data) = line.strip_prefix("data:") {
                return Some(serde_json::from_str(data).unwrap());
            }
            if line == "0\r\n" {
                return None;
            }
        }
    }

    #[tokio::test]
    async fn stream_opens_with_the_status_and_follows_events() {
        let feed = EventFeed::default();
        feed.publish(&AppEvent::StatusUpdated(GameStatus {
            scanning: true,
            ..GameStatus::default()
        }));
        let port = served(&feed).await;

        let (status_line, mut reader) = get_events(port, &format!("localhost:{port}")).await;
        assert_eq!(status_line, "HTTP/1.1 200 OK\r\n");
        let opening = next_event(&mut reader).await.unwrap();
        assert_eq!(opening["event"], "status");
        assert_eq!(opening["data"]["scanning"], true);

        feed.publish(&AppEvent::PricesUpdated);
        assert_eq!(
            next_event(&mut reader).await.unwrap(),
            json!({"event": "prices_updated"})
        );

        feed.configure(EventFeedSettings::default());
        assert_eq!(next_event(&mut reader).await, None);
    }

    #[tokio::test]
    async fn only_local_hosts_are_served() {
        let feed = EventFeed::default();
        let port = served(&feed).await;

        for host in [
            format!("evil.example:{port}"),
            "127.0.0.1".to_owned(),
            String::new(),
        ] {
            let (status_line, _) = get_events(port, &host).await;
            assert_eq!(status_line, "HTTP/1.1 403 Forbidden\r\n", "{host}");
        }
        let (status_line, _) = get_events(port, &format!("127.0.0.1:{port}")).await;
        assert_eq!(status_line, "HTTP/1.1 200 OK\r\n");
    }

    #[test]
    fn status_drops_account_and_machine_details() {
        let status = GameStatus {
            game_detected: true,
            pid: Some(4242),
            log_file: Some("/games/warframe/EE.log".to_owned()),
            last_sync_oid: Some("0123456789abcdef01234567".to_owned()),
            market_account: Some(MarketAccount {
                ingame_name: "TestTenno".to_owned(),
                slug: "testtenno".to_owned(),
                tier: "silver".to_owned(),
                mastery_rank: 14,
            }),
            trades_remaining: Some(3),
            ..GameStatus::default()
        };

        let json = forwarded_json(&AppEvent::StatusUpdated(status)).unwrap();

        assert_eq!(json["event"], "status");
        assert_eq!(json["data"]["game_detected"], true);
        assert_eq!(json["data"]["trades_remaining"], 3);
        for hidden in ["pid", "log_file", "last_sync_oid", "market_account"] {
            assert!(json["data"].get(hidden).is_none());
        }
    }

    #[test]
    fn trade_keeps_the_items_and_drops_the_partner() {
        let event = AppEvent::CoreEvent(CoreEventEnvelope::wrap(CoreEvent::TradeCompleted {
            at: Utc::now(),
            partner: Some("TestSquadA".to_owned()),
            trade: Trade {
                offered: vec![TradeItem {
                    name: "Saryn Prime Set".to_owned(),
                    count: 1,
                    rank: None,
                }],
                received: vec![],
                plat: 80,
            },
        }));

        let json = forwarded_json(&event).unwrap();

        assert_eq!(json["event"], "trade_completed");
        assert_eq!(json["data"]["trade"]["plat"], 80);
        assert_eq!(
            json["data"]["trade"]["offered"],
            json!([{"name": "Saryn Prime Set", "count": 1, "rank": null}])
        );
        assert!(json["data"].get("partner").is_none());
    }

    #[test]
    fn internal_and_chat_events_stay_inside() {
        let conversation = CoreEvent::NewConversation {
            channel: "F".to_owned(),
            player: "TestSquadB".to_owned(),
        };
        let inside = [
            AppEvent::CoreEvent(CoreEventEnvelope::wrap(conversation)),
            AppEvent::InventoryUpdated(GameStatus::default()),
            AppEvent::MarketUpdated(MarketSnapshot {
                orders: None,
                auctions: None,
                at: Utc::now(),
            }),
            AppEvent::MarketSessionRejected,
        ];

        for event in &inside {
            assert!(FeedEvent::forwarded(event).is_none());
        }
    }

    #[test]
    fn inventory_summary_drops_the_sync_id() {
        let summary = InventorySummary {
            last_sync_oid: "0123456789abcdef01234567".to_owned(),
            mr: 14,
            plat: 120,
            credits: 50_000,
            endo: 3_000,
            ducats: 40,
            changes: 2,
        };
        let event = AppEvent::CoreEvent(CoreEventEnvelope::wrap(CoreEvent::InventoryUpdated(
            summary,
        )));

        assert_eq!(
            forwarded_json(&event).unwrap(),
            json!({"event": "inventory_updated", "data": {
                "mr": 14, "plat": 120, "credits": 50000, "endo": 3000, "ducats": 40, "changes": 2
            }})
        );
    }

    #[test]
    fn bare_events_carry_only_their_name() {
        assert_eq!(
            forwarded_json(&AppEvent::PricesUpdated).unwrap(),
            json!({"event": "prices_updated"})
        );
    }

    #[test]
    fn unchanged_status_is_sent_once() {
        let feed = EventFeed::default();
        let mut client = feed.events.subscribe();
        let status = AppEvent::StatusUpdated(GameStatus::default());

        feed.publish(&status);
        feed.publish(&status);
        feed.publish(&AppEvent::StatusUpdated(GameStatus {
            scanning: true,
            ..GameStatus::default()
        }));
        feed.publish(&AppEvent::PricesUpdated);
        feed.publish(&AppEvent::PricesUpdated);

        let mut received = Vec::new();
        while let Ok(event) = client.try_recv() {
            received.push(serde_json::to_value(event).unwrap()["event"].clone());
        }
        assert_eq!(
            received,
            ["status", "status", "prices_updated", "prices_updated"]
        );
    }

    #[test]
    fn status_is_remembered_while_nobody_listens() {
        let feed = EventFeed::default();
        let scanning = AppEvent::StatusUpdated(GameStatus {
            scanning: true,
            ..GameStatus::default()
        });
        let idle = AppEvent::StatusUpdated(GameStatus::default());

        feed.publish(&scanning);
        feed.publish(&idle);
        let mut client = feed.events.subscribe();
        feed.publish(&scanning);

        let event = client.try_recv().unwrap();
        assert!(matches!(event, FeedEvent::Status(status) if status.scanning));
    }

    #[test]
    fn settings_decide_the_port() {
        let feed = EventFeed::default();
        assert_eq!(*feed.wanted.borrow(), None);

        let mut wanted = feed.wanted.subscribe();
        let enabled = EventFeedSettings {
            event_feed_enabled: true,
            event_feed_port: 28005,
        };
        feed.configure(enabled);
        assert_eq!(*wanted.borrow_and_update(), Some(28005));

        feed.configure(enabled);
        assert!(!wanted.has_changed().unwrap());

        feed.configure(EventFeedSettings::default());
        assert_eq!(*wanted.borrow_and_update(), None);
    }
}
