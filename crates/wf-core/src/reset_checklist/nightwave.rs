use std::collections::HashSet;

use wf_inventory::NightwaveWeek;
use wf_worldstate::{
    ChallengeKind, NightwaveChallenge, NightwaveSeason, ResetWindow, nightwave_act_definition,
};

use crate::reset_checklist::ResetChecklistRowPart;
use crate::reset_checklist::cadence::ResetCadence;
use crate::reset_checklist::context::ResetChecklistContext;
use crate::reset_checklist::trackers::{Tracked, tally};
use crate::text::counted;

const RECOVERED_ACT_SLOTS: usize = 3;

fn current_act_parts(
    context: &ResetChecklistContext,
    season: &NightwaveSeason,
    cadence: ResetCadence,
) -> Vec<ResetChecklistRowPart> {
    season
        .challenges
        .iter()
        .filter(|challenge| {
            (challenge.act.kind == ChallengeKind::Daily) == (cadence == ResetCadence::Daily)
        })
        .map(|challenge| {
            let counter_stale = !context.synced_within(ResetWindow {
                activation: challenge.activation,
                expiry: challenge.expiry,
            });
            let progress = if counter_stale {
                0
            } else {
                context
                    .inventory
                    .challenge_progress_of(challenge.counter_name())
            };
            ResetChecklistRowPart::new(challenge.act.name.clone(), progress, challenge.act.required)
        })
        .collect()
}

pub(crate) fn active_acts(
    context: &ResetChecklistContext,
    season: &NightwaveSeason,
    cadence: ResetCadence,
) -> Tracked {
    let parts = current_act_parts(context, season, cadence);

    let completed = tally(parts.iter().filter(|part| part.completed));

    Tracked::counted(completed, tally(&parts)).with_parts(parts)
}

fn recovered_acts_offered(open_current_weekly: usize, pool: usize) -> usize {
    RECOVERED_ACT_SLOTS
        .saturating_sub(open_current_weekly)
        .min(pool)
}

fn reserve_note(reserve: usize) -> Option<String> {
    (reserve > 0).then(|| format!("{} on reserve", counted(reserve, "act")))
}

pub(crate) fn recovered_acts(
    context: &ResetChecklistContext,
    season: &NightwaveSeason,
) -> Option<Tracked> {
    let inventory = context.inventory;
    let current = season
        .challenges
        .iter()
        .filter_map(|challenge| NightwaveWeek::of_act(&challenge.id))
        .max()?;
    let active: HashSet<&str> = season
        .challenges
        .iter()
        .map(NightwaveChallenge::counter_name)
        .collect();

    let mut given: Vec<_> = inventory
        .nightwave_acts_given_before(current)
        .filter(|act| !active.contains(act.challenge.as_str()))
        .collect();
    given.sort_by(|left, right| right.id.cmp(&left.id));
    let mut named = HashSet::new();
    given.retain(|act| named.insert(act.challenge.as_str()));
    given.reverse();
    let mut pool: Vec<_> = given
        .into_iter()
        .map(|act| {
            let text = context.catalog.nightwave_act_text(&act.challenge);
            (act, nightwave_act_definition(&act.challenge, text))
        })
        .filter(|(_, definition)| definition.kind != ChallengeKind::Daily)
        .map(|(act, definition)| {
            ResetChecklistRowPart::new(
                definition.name,
                inventory.challenge_progress_of(&act.challenge),
                definition.required,
            )
        })
        .filter(|part| !part.completed)
        .collect();

    let open_current = current_act_parts(context, season, ResetCadence::Weekly)
        .iter()
        .filter(|part| !part.completed)
        .count();
    let reserve = pool.split_off(recovered_acts_offered(open_current, pool.len()));

    Some(
        Tracked::finished(pool.is_empty() && reserve.is_empty())
            .with_detail(reserve_note(reserve.len()))
            .with_parts(pool),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovered_slots() {
        let offered = |open: usize| {
            [0, 1, 4]
                .into_iter()
                .map(|pool| recovered_acts_offered(open, pool))
                .collect::<Vec<_>>()
        };

        assert_eq!(offered(0), [0, 1, 3]);
        assert_eq!(offered(1), [0, 1, 2]);
        assert_eq!(offered(2), [0, 1, 1]);
        assert_eq!(offered(3), [0, 0, 0]);
        assert_eq!(offered(5), [0, 0, 0]);
    }

    #[test]
    fn reserve_notes() {
        assert_eq!(reserve_note(0), None);
        assert_eq!(reserve_note(1).as_deref(), Some("1 act on reserve"));
        assert_eq!(reserve_note(3).as_deref(), Some("3 acts on reserve"));
    }
}
