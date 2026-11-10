import { useMemo } from "react";
import { ArbitrationLiveOverlay } from "@/components/arbitration-live-overlay";
import { DucateringOverlay } from "@/components/ducatering-overlay";
import { EmptyNote } from "@/components/page";
import {
  RelicRecommendationOverlay,
  useOverlayFilters,
} from "@/components/relic-recommendation-overlay";
import { RelicRewardOverlay } from "@/components/relic-reward-overlay";
import { RivenOverlay, RivenRerollOverlay } from "@/components/riven-overlay";
import { useLoaded } from "@/hooks/use-loaded";
import { api, logError } from "@/lib/bridge";
import { DEFAULT_SQUAD_SIZE } from "@/stores/preferences-store";
import type {
  ArbitrationRun,
  DucateringProgress,
  RecommendationTrigger,
  RewardTrigger,
  RivenTrigger,
} from "@/types";

export function RewardSlot({
  trigger,
  compact = false,
}: {
  trigger: RewardTrigger | null;
  compact?: boolean;
}) {
  if (!trigger) {
    return <EmptyNote>Open a relic to see squad rewards and prices.</EmptyNote>;
  }
  return <RelicRewardOverlay screen={trigger.screen} compact={compact} />;
}

export function RecommendationSlot({
  trigger,
  compact = false,
}: {
  trigger: RecommendationTrigger | null;
  compact?: boolean;
}) {
  const shown = trigger !== null;
  const tier = trigger?.tier ?? null;
  const filters = useOverlayFilters();
  const squadSize = filters?.squadSize ?? DEFAULT_SQUAD_SIZE;
  const wanted = filters?.wanted;
  const reloadKey = useMemo(
    () => (shown ? { tier, squadSize, wanted: wanted ?? [] } : null),
    [shown, tier, squadSize, wanted],
  );
  const plans = useLoaded(
    reloadKey,
    (key) => api.relicPlans(key.tier, key.squadSize, key.wanted),
    (cause) => {
      logError("Ranking the owned relics", cause);
      return [];
    },
    ["favouriteUpdated", "inventoryUpdated", "pricesUpdated"],
  );

  if (!trigger) {
    return <EmptyNote>Choose a fissure to rank your relics for it.</EmptyNote>;
  }
  if (plans === null) {
    return <EmptyNote>Reading the owned relics.</EmptyNote>;
  }
  return (
    <RelicRecommendationOverlay
      tier={trigger.tier}
      plans={plans}
      refinement={trigger.refinement}
      limit={trigger.count}
      filters={filters}
      squadSize={squadSize}
      compact={compact}
    />
  );
}

export function RivenSlot({
  trigger,
  compact = false,
}: {
  trigger: RivenTrigger | null;
  compact?: boolean;
}) {
  if (!trigger) {
    return (
      <EmptyNote>
        Open a riven or the reroll station to compare rolls.
      </EmptyNote>
    );
  }
  const { before, linked, attribution } = trigger;
  if (before) {
    const after = before.pending
      ? { ...before, ...before.pending, pending: null }
      : null;
    return (
      <RivenRerollOverlay
        before={before}
        after={after}
        attribution={attribution}
        compact={compact}
      />
    );
  }
  if (!linked) {
    return (
      <EmptyNote>
        The riven in the dialog could not be read from the game.
      </EmptyNote>
    );
  }
  return (
    <RivenOverlay
      rivens={[linked]}
      attribution={attribution}
      compact={compact}
    />
  );
}

export function ArbitrationLiveSlot({
  trigger,
}: {
  trigger: ArbitrationRun | null;
}) {
  if (!trigger) {
    return (
      <EmptyNote>
        No Arbitration is running. Test shows the last recorded run.
      </EmptyNote>
    );
  }
  return <ArbitrationLiveOverlay run={trigger} />;
}

export function DucateringSlot({
  trigger,
  clipboard,
  compact = false,
}: {
  trigger: DucateringProgress | null;
  clipboard: boolean;
  compact?: boolean;
}) {
  if (!trigger) {
    return (
      <EmptyNote>
        Open the Ducat Kiosk in a relay to start Ducatering.
      </EmptyNote>
    );
  }
  return (
    <DucateringOverlay
      progress={trigger}
      clipboard={clipboard}
      compact={compact}
    />
  );
}
