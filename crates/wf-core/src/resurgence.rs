use std::collections::HashSet;

use chrono::{DateTime, Utc};
use wf_data::store_item_to_type;
use wf_worldstate::WorldState;

pub(crate) fn prime_resurgence(world: Option<&WorldState>, now: DateTime<Utc>) -> HashSet<String> {
    let Some(varzia) = world.and_then(|world| world.varzia(now)) else {
        return HashSet::new();
    };
    varzia
        .items
        .iter()
        .map(|offer| store_item_to_type(&offer.item_type))
        .collect()
}
