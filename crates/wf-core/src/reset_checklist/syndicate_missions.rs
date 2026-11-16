use std::collections::HashSet;
use std::ops::Range;

use wf_inventory::Inventory;
use wf_worldstate::SyndicateMissionSet;

use crate::reset_checklist::ResetChecklistRowPart;
use crate::reset_checklist::trackers::{Tracked, tally};

const MISSIONS_PER_RANK: usize = 3;

fn faction_syndicate_name(tag: &str) -> Option<&'static str> {
    match tag {
        "SteelMeridianSyndicate" => Some("Steel Meridian"),
        "ArbitersSyndicate" => Some("Arbiters of Hexis"),
        "CephalonSudaSyndicate" => Some("Cephalon Suda"),
        "PerrinSyndicate" => Some("The Perrin Sequence"),
        "RedVeilSyndicate" => Some("Red Veil"),
        "NewLokaSyndicate" => Some("New Loka"),
        _ => None,
    }
}

fn missions_open_at_rank(rank: i32) -> Range<usize> {
    match usize::try_from(rank) {
        Ok(rank) if rank > 0 => rank - 1..rank - 1 + MISSIONS_PER_RANK,
        _ => 0..0,
    }
}

pub(crate) fn syndicate_missions_open(inventory: &Inventory) -> bool {
    inventory.affiliations.iter().any(|affiliation| {
        faction_syndicate_name(&affiliation.tag).is_some()
            && !missions_open_at_rank(affiliation.title).is_empty()
    })
}

fn syndicate_part(
    inventory: &Inventory,
    set: &SyndicateMissionSet,
) -> Option<ResetChecklistRowPart> {
    let name = faction_syndicate_name(&set.tag)?;
    let rank = inventory
        .affiliations
        .iter()
        .find(|affiliation| affiliation.tag == set.tag)?
        .title;
    let open = missions_open_at_rank(rank);
    if open.is_empty() {
        return None;
    }

    let completed: HashSet<&str> = inventory.syndicate_nodes_completed(&set.id).collect();
    let nodes: Vec<_> = set.nodes.iter().take(open.end).skip(open.start).collect();
    let done = tally(
        nodes
            .iter()
            .filter(|node| completed.contains(node.as_str())),
    );

    Some(ResetChecklistRowPart::new(
        name.to_owned(),
        done,
        tally(&nodes),
    ))
}

pub(crate) fn syndicate_missions(
    inventory: &Inventory,
    sets: &[SyndicateMissionSet],
) -> Option<Tracked> {
    if sets.is_empty() {
        return None;
    }

    let parts: Vec<_> = sets
        .iter()
        .filter_map(|set| syndicate_part(inventory, set))
        .collect();

    let done = parts.iter().map(|part| part.done).sum();
    let total = parts.iter().map(|part| part.total).sum();

    Some(Tracked::counted(done, total).with_parts(parts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::fixtures;

    #[test]
    fn rank_window() {
        assert_eq!(missions_open_at_rank(-2), 0..0);
        assert_eq!(missions_open_at_rank(0), 0..0);
        assert_eq!(missions_open_at_rank(1), 0..3);
        assert_eq!(missions_open_at_rank(3), 2..5);
        assert_eq!(missions_open_at_rank(5), 4..7);
    }

    #[test]
    fn short_set() {
        let set = SyndicateMissionSet {
            tag: "PerrinSyndicate".to_owned(),
            id: "6a9edf461c3038a4ecd9dc67".to_owned(),
            nodes: [
                "SolNode45",
                "SolNode61",
                "SolNode103",
                "SolNode141",
                "SolNode64",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        };

        let tracked = syndicate_missions(&fixtures::inventory(), &[set]).unwrap();

        assert_eq!(
            tracked.parts,
            [ResetChecklistRowPart::new(
                "The Perrin Sequence".to_owned(),
                0,
                1
            )]
        );
    }

    #[test]
    fn no_running_sets() {
        assert_eq!(syndicate_missions(&fixtures::inventory(), &[]), None);
    }
}
