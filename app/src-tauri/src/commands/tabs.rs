use chrono::Utc;
use tauri::{AppHandle, Runtime};
use wf_core::{
    ComparedStat, CraftDetails, DeltaRow, FoundryTab, InventoryTab, MasteryOptions,
    MasteryOrdering, MasteryTab, OpeningRow, RelicPlan, RelicPlannerTab, RelicSource,
    ResetChecklistManualRowId, ResetChecklistTab, ResourceQuery, ResourcesTab, RivenComparables,
    RivensTab, StatsTab, StoredTrade,
};

use super::{Shared, compute, missing_inventory, ready, since};
use crate::error::CommandResult;
use crate::runtime::{self, AppEvent};
use crate::state::{lock, read};

#[tauri::command]
pub async fn inventory_tab(state: Shared<'_>) -> CommandResult<InventoryTab> {
    let state = ready(&state).await?;
    compute(state, wf_core::Core::inventory_tab).await
}

#[tauri::command]
pub async fn foundry_tab(state: Shared<'_>) -> CommandResult<FoundryTab> {
    let state = ready(&state).await?;
    let include_founders = read(&state.settings).inventory.include_founders_items;
    compute(state, move |core| {
        core.foundry_tab(include_founders, Utc::now())
    })
    .await
}

#[tauri::command]
pub async fn craft_tree(state: Shared<'_>, unique_name: String) -> CommandResult<CraftDetails> {
    let state = ready(&state).await?;
    compute(state, move |core| core.craft_tree(&unique_name)).await
}

#[tauri::command]
pub async fn mastery_tab(
    state: Shared<'_>,
    ordering: Option<MasteryOrdering>,
    include_founders: Option<bool>,
    include_forma_ranks: Option<bool>,
) -> CommandResult<MasteryTab> {
    let state = ready(&state).await?;
    let stored = read(&state.settings).mastery_options(ordering.unwrap_or_default());
    let options = MasteryOptions {
        ordering: stored.ordering,
        include_founders_items: include_founders.or(stored.include_founders_items),
        include_forma_ranks: include_forma_ranks.unwrap_or(stored.include_forma_ranks),
    };
    compute(state, move |core| core.mastery_tab(options)).await
}

#[tauri::command]
pub async fn resources_tab(state: Shared<'_>, query: ResourceQuery) -> CommandResult<ResourcesTab> {
    let state = ready(&state).await?;
    let include_founders = read(&state.settings).inventory.include_founders_items;
    compute(state, move |core| {
        core.resources_tab(&query, include_founders, Utc::now())
    })
    .await
}

#[tauri::command]
pub async fn relic_planner_tab(
    state: Shared<'_>,
    squad_size: Option<u32>,
    only_owned: Option<bool>,
    wanted: Vec<String>,
) -> CommandResult<RelicPlannerTab> {
    let state = ready(&state).await?;
    let squad = squad_size.unwrap_or(wf_core::DEFAULT_SQUAD_SIZE);
    let owned = only_owned.unwrap_or(true);
    compute(state, move |core| {
        core.relic_planner_tab(squad, owned, &wanted)
    })
    .await
}

#[tauri::command]
pub async fn relic_plans(
    state: Shared<'_>,
    tier: Option<String>,
    squad_size: u32,
    wanted: Vec<String>,
) -> CommandResult<Vec<RelicPlan>> {
    let state = ready(&state).await?;
    compute(state, move |core| {
        core.relic_plans(tier.as_deref(), squad_size, &wanted)
    })
    .await
}

#[tauri::command]
pub async fn rivens_tab(state: Shared<'_>) -> CommandResult<RivensTab> {
    let state = ready(&state).await?;
    compute(state, wf_core::Core::rivens_tab).await
}

#[tauri::command]
pub async fn riven_comparables(
    state: Shared<'_>,
    weapon_slug: String,
    shown: Vec<ComparedStat>,
) -> CommandResult<Option<RivenComparables>> {
    let state = ready(&state).await?;
    let Some(auctions) = state.auctions.weapon(&state.http, &weapon_slug).await? else {
        return Ok(None);
    };
    Ok(Some(lock(&state.core).riven_comparables(&shown, &auctions)))
}

#[tauri::command]
pub async fn stats_tab(state: Shared<'_>, since_ms: Option<i64>) -> CommandResult<StatsTab> {
    let state = ready(&state).await?;
    Ok(lock(&state.core).stats_tab(since(since_ms), Utc::now())?)
}

#[tauri::command]
pub async fn stats_deltas(
    state: Shared<'_>,
    since_ms: Option<i64>,
) -> CommandResult<Vec<DeltaRow>> {
    let state = ready(&state).await?;
    Ok(lock(&state.core).stats_deltas(since(since_ms))?)
}

#[tauri::command]
pub async fn stats_openings(
    state: Shared<'_>,
    since_ms: Option<i64>,
) -> CommandResult<Vec<OpeningRow>> {
    let state = ready(&state).await?;
    Ok(lock(&state.core).relic_openings(since(since_ms))?)
}

#[tauri::command]
pub async fn stats_trades(
    state: Shared<'_>,
    since_ms: Option<i64>,
) -> CommandResult<Vec<StoredTrade>> {
    let state = ready(&state).await?;
    Ok(lock(&state.core).store().trades(since(since_ms))?)
}

#[tauri::command]
pub async fn reset_checklist_tab(state: Shared<'_>) -> CommandResult<ResetChecklistTab> {
    let state = ready(&state).await?;
    lock(&state.core)
        .reset_checklist_tab(Utc::now())?
        .ok_or_else(missing_inventory)
}

#[tauri::command]
pub async fn set_reset_checklist_checked<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    id: ResetChecklistManualRowId,
    checked: bool,
) -> CommandResult<()> {
    let state = ready(&state).await?;
    lock(&state.core).set_reset_checklist_checked(id, checked, Utc::now())?;
    runtime::emit_with_reset_checklist(&app, &state, AppEvent::ResetChecklistUpdated);
    Ok(())
}

#[tauri::command]
pub async fn toggle_favourite<R: Runtime>(
    app: AppHandle<R>,
    state: Shared<'_>,
    unique_name: String,
) -> CommandResult<bool> {
    let state = ready(&state).await?;
    let favourite = lock(&state.core).toggle_favourite(&unique_name)?;
    runtime::emit(&app, AppEvent::FavouriteUpdated);
    Ok(favourite)
}

#[tauri::command]
pub async fn relics_for(
    state: Shared<'_>,
    part_unique_name: String,
) -> CommandResult<Vec<RelicSource>> {
    let state = ready(&state).await?;
    compute(state, move |core| core.relics_for(&part_unique_name)).await
}
