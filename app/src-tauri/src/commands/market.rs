use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use wf_core::{ListingChoices, market_name, riven_listing_payload, riven_listing_update};
use wf_market::{
    Auction, CreateOrderRequest, ItemListings, Order, OrderType, UpdateAuctionRequest,
    UpdateOrderRequest, order_rejection,
};

use super::{Shared, ready};

use crate::error::{CommandError, CommandResult};
use crate::market::{self, MarketItems, MarketOrders, MarketPresence, MarketPresenceChoice};
use crate::market_session;
use crate::runtime::{self, AppEvent};
use crate::settings::MarketAccount;
use crate::state::{AppState, lock, read};

fn unlisted_items() -> CommandError {
    CommandError::from("The warframe.market item list is not loaded")
}

pub(super) async fn listed_items(state: &Arc<AppState>) -> CommandResult<Arc<MarketItems>> {
    market::market_items(state).await.ok_or_else(unlisted_items)
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export))]
pub struct MarketItem {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub image_name: Option<String>,
    pub ducats: Option<u32>,
    pub tradable: Option<bool>,
    pub bulk_tradable: Option<bool>,
    pub max_rank: Option<u32>,
    pub tags: Vec<String>,
    pub subtypes: Vec<String>,
    pub max_amber_stars: Option<u32>,
    pub max_cyan_stars: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct NewOrder {
    pub item_id: String,
    pub order_type: OrderType,
    pub platinum: u32,
    pub quantity: u32,
    pub visible: Option<bool>,
    pub per_trade: Option<u32>,
    pub rank: Option<u32>,
    pub subtype: Option<String>,
    pub amber_stars: Option<u32>,
    pub cyan_stars: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct OrderPatch {
    pub platinum: Option<u32>,
    pub quantity: Option<u32>,
    pub visible: Option<bool>,
}

#[tauri::command]
pub async fn market_login<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    email: String,
    password: String,
) -> CommandResult<MarketAccount> {
    let state = ready(&state).await?;

    let token = state.market().login(&email, &password).await?;
    let probe = state.anonymous_market().with_token(token.clone());
    let session = probe.me().await?;
    if !session.user.verification {
        return Err(CommandError::from(
            "warframe.market has not verified this account yet, confirm the email it sent you",
        ));
    }

    market_session::store_token(&app, &state, &token)?;
    let account = MarketAccount::from(&session);
    market_session::record_account(&app, &state, &account)?;
    Ok(account)
}

#[tauri::command]
pub async fn market_logout<R: Runtime>(app: AppHandle<R>, state: Shared<'_>) -> CommandResult<()> {
    let state = ready(&state).await?;
    market_session::clear(&app, &state)?;
    Ok(())
}

#[tauri::command]
pub async fn market_items(state: Shared<'_>) -> CommandResult<Vec<MarketItem>> {
    let state = ready(&state).await?;

    let listed = listed_items(&state).await?;

    let core = lock(&state.core);
    let mut items: Vec<MarketItem> = listed
        .items()
        .iter()
        .map(|item| MarketItem {
            name: market_name(item).to_owned(),
            image_name: core.items().market_icon(item),
            id: item.id.clone(),
            slug: item.slug.clone(),
            ducats: item.ducats,
            tradable: item.tradable,
            bulk_tradable: item.bulk_tradable,
            max_rank: item.max_rank,
            tags: item.tags.clone(),
            subtypes: item.subtypes.clone().unwrap_or_default(),
            max_amber_stars: item.max_amber_stars,
            max_cyan_stars: item.max_cyan_stars,
        })
        .collect();
    items.sort_unstable_by(|a, b| a.name.cmp(&b.name));

    Ok(items)
}

#[tauri::command]
pub async fn market_item_orders(state: Shared<'_>, slug: String) -> CommandResult<ItemListings> {
    let state = ready(&state).await?;
    let reach = read(&state.settings).trader_reach();
    Ok(state.market().item_listings(&slug, &reach).await?)
}

#[tauri::command]
pub async fn market_my_orders(state: Shared<'_>) -> CommandResult<MarketOrders> {
    let state = ready(&state).await?;

    let orders = state.market().orders_my().await?;
    let rows = market::order_rows(&state, &orders)
        .await
        .ok_or_else(unlisted_items)?;

    state.listings.remember(&state.core, Some(&rows), None);
    Ok(rows)
}

#[tauri::command]
pub async fn market_post_order<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    order: NewOrder,
) -> CommandResult<Order> {
    let state = ready(&state).await?;

    let body = CreateOrderRequest {
        item_id: order.item_id,
        order_type: order.order_type,
        platinum: order.platinum,
        quantity: order.quantity,
        visible: order.visible,
        per_trade: order.per_trade,
        rank: order.rank,
        subtype: order.subtype,
        amber_stars: order.amber_stars,
        cyan_stars: order.cyan_stars,
    };
    let posted =
        state.market().create_order(&body).await.map_err(|error| {
            match order_rejection(&error) {
                Some(cause) => CommandError::from(cause),
                None => error.into(),
            }
        })?;

    publish_orders(&app, &state).await;

    Ok(posted)
}

#[tauri::command]
pub async fn market_update_order<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    id: String,
    patch: OrderPatch,
) -> CommandResult<Order> {
    let state = ready(&state).await?;

    let body = UpdateOrderRequest {
        platinum: patch.platinum,
        quantity: patch.quantity,
        visible: patch.visible,
    };
    let updated = state.market().update_order(&id, &body).await?;
    publish_orders(&app, &state).await;

    Ok(updated)
}

async fn publish_orders<R: Runtime>(app: &AppHandle<R>, state: &Arc<AppState>) {
    let orders = match state.market().orders_my().await {
        Ok(orders) => orders,
        Err(error) => {
            tracing::warn!(error = %error.brief(), "Order refresh after a write failed");
            return;
        }
    };
    let snapshot = market::market_snapshot(state, Some(&orders), None).await;
    runtime::emit(app, AppEvent::MarketUpdated(snapshot));
}

#[tauri::command]
pub async fn market_close_order(state: Shared<'_>, id: String, quantity: u32) -> CommandResult<()> {
    let state = ready(&state).await?;
    Ok(state.market().close_order(&id, quantity).await?)
}

#[tauri::command]
pub async fn market_delete_order(state: Shared<'_>, id: String) -> CommandResult<Order> {
    let state = ready(&state).await?;
    Ok(state.market().delete_order(&id).await?)
}

#[tauri::command]
pub async fn market_fix_orders(state: Shared<'_>, id: Option<String>) -> CommandResult<u32> {
    let state = ready(&state).await?;

    let client = state.market();
    let orders = client.orders_my().await?;
    let listed = market::order_rows(&state, &orders)
        .await
        .ok_or_else(unlisted_items)?;

    let wanted = listed
        .rows
        .iter()
        .filter(|row| row.show_warning)
        .filter(|row| id.as_ref().is_none_or(|wanted| wanted == &row.id));
    let mut fixed = 0;
    for row in wanted {
        let repaired = match u32::try_from(row.owned) {
            Ok(0) => client.delete_order(&row.id).await.map(|_| ()),
            Ok(owned) => {
                let patch = UpdateOrderRequest {
                    quantity: Some(owned),
                    ..UpdateOrderRequest::default()
                };
                client.update_order(&row.id, &patch).await.map(|_| ())
            }
            Err(_) => continue,
        };

        match repaired {
            Ok(()) => fixed += 1,
            Err(error) => {
                tracing::warn!(
                    order = row.id,
                    owned = row.owned,
                    %error,
                    "Order quantity fix failed",
                );
            }
        }
    }

    Ok(fixed)
}

#[tauri::command]
pub async fn market_remove_all(state: Shared<'_>) -> CommandResult<u32> {
    let state = ready(&state).await?;

    let client = state.market();
    let orders = client.orders_my().await?;

    let mut closed = 0;
    for order in orders {
        match client.delete_order(&order.id).await {
            Ok(_) => closed += 1,
            Err(error) => {
                tracing::warn!(
                    order = order.id,
                    %error,
                    "Order delete failed",
                );
            }
        }
    }

    Ok(closed)
}

#[tauri::command]
pub async fn market_set_visibility(state: Shared<'_>, visible: bool) -> CommandResult<u32> {
    let state = ready(&state).await?;
    Ok(state
        .market()
        .set_all_orders_visibility(visible)
        .await?
        .updated)
}

#[tauri::command]
pub async fn market_post_riven(
    state: Shared<'_>,
    item_id: String,
    choices: ListingChoices,
) -> CommandResult<String> {
    let state = ready(&state).await?;

    let payload = {
        let core = lock(&state.core);
        let row = core
            .riven_row(&item_id)
            .ok_or_else(|| CommandError::from("This riven is not in the current inventory"))?;
        riven_listing_payload(&row, &choices).ok_or_else(|| {
            CommandError::from("This riven has no warframe.market attribute mapping")
        })?
    };

    Ok(state.market().create_auction(&payload).await?.id)
}

#[tauri::command]
pub async fn market_my_auctions(state: Shared<'_>) -> CommandResult<Vec<Auction>> {
    let state = ready(&state).await?;

    let slug = state
        .market_slug()
        .ok_or_else(|| CommandError::from("Sign in to warframe.market first"))?;
    let auctions = state.market().auctions_my(&slug).await?;

    state.listings.remember(&state.core, None, Some(&auctions));
    Ok(auctions)
}

#[tauri::command]
pub async fn market_edit_auction(
    state: Shared<'_>,
    id: String,
    choices: ListingChoices,
) -> CommandResult<Auction> {
    let state = ready(&state).await?;
    let body = riven_listing_update(&choices);
    Ok(state.market().update_auction(&id, &body).await?)
}

#[tauri::command]
pub async fn market_set_auction_visible(
    state: Shared<'_>,
    id: String,
    visible: bool,
) -> CommandResult<Auction> {
    let state = ready(&state).await?;
    let body = UpdateAuctionRequest {
        visible: Some(visible),
        ..UpdateAuctionRequest::default()
    };
    Ok(state.market().update_auction(&id, &body).await?)
}

#[tauri::command]
pub async fn market_close_auction(state: Shared<'_>, id: String) -> CommandResult<()> {
    let state = ready(&state).await?;
    Ok(state.market().close_auction(&id).await?)
}

#[tauri::command]
pub async fn market_set_auctions_visibility(state: Shared<'_>, visible: bool) -> CommandResult<()> {
    let state = ready(&state).await?;
    Ok(state.market().set_auctions_visibility(visible).await?)
}

#[tauri::command]
pub async fn market_presence(state: Shared<'_>) -> CommandResult<MarketPresence> {
    let state = ready(&state).await?;
    Ok(*read(&state.market_presence))
}

#[tauri::command]
pub async fn market_set_presence<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    choice: MarketPresenceChoice,
) -> CommandResult<MarketPresence> {
    let state = ready(&state).await?;
    Ok(market_session::set_presence(&app, &state, choice)?)
}

#[tauri::command]
pub async fn market_activity(state: Shared<'_>) -> CommandResult<()> {
    let state = ready(&state).await?;
    *lock(&state.market_activity) = Some(Instant::now());
    Ok(())
}
