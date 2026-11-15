import { OptionTabs } from "@/components/option-tabs";
import { CardGrid, EmptyNote, Page, Section } from "@/components/page";
import { useNow } from "@/hooks/use-now";
import { useRollover } from "@/hooks/use-rollover";
import { api, reportError } from "@/lib/bridge";
import { secondsUntil } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/app-store";
import { usePreferencesStore } from "@/stores/preferences-store";
import type { Fissure, FissurePath } from "@/types";
import { WorldArbitrationCard } from "./arbitrations";
import { FissuresByTier } from "./fissures";
import { MarketSalesPanel } from "./market-sales";
import {
  ArchonPanel,
  BaroPanel,
  CircuitPanel,
  DarvoPanel,
  NightwavePanel,
  ResetsPanel,
  ResurgencePanel,
  SortiePanel,
} from "./panels";
import { WorldRewardAlertsPanel } from "./rewards";
import { TimerChip } from "./timers";

const PATH_FILTERS: { value: FissurePath; label: string }[] = [
  { value: "baseline", label: "Normal + Steel Path" },
  { value: "normal", label: "Normal" },
  { value: "hard", label: "Steel Path" },
  { value: "storm", label: "Void Storms" },
  { value: "all", label: "All" },
];

export function WorldPage() {
  const data = useAppStore((state) => state.world);
  const setWorld = useAppStore((state) => state.setWorld);
  const now = useNow();
  const fissurePath = usePreferencesStore((state) => state.fissurePath);
  const setFissurePath = usePreferencesStore((state) => state.setFissurePath);

  const rollovers = data
    ? [
        ...data.timers.map((timer) => timer.ends),
        data.daily_reset.expiry,
        data.weekly_reset.expiry,
      ]
    : [];
  useRollover(rollovers, now, () => {
    api.worldstate().then(setWorld, reportError);
  });

  const shown: Record<FissurePath, (fissure: Fissure) => boolean> = {
    all: () => true,
    baseline: (fissure) => !fissure.is_storm,
    normal: (fissure) => !fissure.steel_path && !fissure.is_storm,
    hard: (fissure) => fissure.steel_path,
    storm: (fissure) => fissure.is_storm,
  };
  const fissures = (data?.fissures ?? [])
    .filter((fissure) => secondsUntil(fissure.expiry, now) > 0)
    .filter(shown[fissurePath])
    .sort(
      (left, right) =>
        new Date(left.expiry).getTime() - new Date(right.expiry).getTime(),
    );

  const sortie = data?.sortie ?? null;
  const baro = data?.baro ?? null;
  const baroPresent = baro !== null && "Present" in baro;
  const baroPanel = (
    <BaroPanel baro={baro} manifest={data?.baro_manifest ?? []} now={now} />
  );

  return (
    <Page title="World" quote="world">
      {data?.timers.length ? (
        <CardGrid>
          {data.timers.map((timer) => (
            <TimerChip key={timer.name} timer={timer} now={now} />
          ))}
          <WorldArbitrationCard now={now} />
        </CardGrid>
      ) : (
        <EmptyNote>The world state has not been fetched yet.</EmptyNote>
      )}

      <Section
        title="Void Fissures"
        description={`${fissures.length} active`}
        action={
          <OptionTabs
            value={fissurePath}
            options={PATH_FILTERS}
            onChange={setFissurePath}
          />
        }
      >
        <FissuresByTier fissures={fissures} now={now} />
      </Section>

      <div
        className={cn(
          "grid items-start gap-4 @lg:grid-cols-2",
          baroPresent && "@5xl:grid-cols-3",
        )}
      >
        {baroPresent && baroPanel}
        <div className="flex flex-col gap-4">
          {!baroPresent && baroPanel}
          <ResetsPanel
            daily={data?.daily_reset ?? null}
            weekly={data?.weekly_reset ?? null}
            sortie={sortie}
            now={now}
          />
          <WorldRewardAlertsPanel
            rewards={data?.world_reward_alerts ?? []}
            now={now}
          />
          <DarvoPanel deals={data?.daily_deals ?? []} now={now} />
          <CircuitPanel circuit={data?.circuit ?? null} now={now} />
          <SortiePanel sortie={sortie} now={now} />
          <ArchonPanel hunt={data?.archon_hunt ?? null} now={now} />
        </div>
        <div className="flex flex-col gap-4">
          <ResurgencePanel
            resurgence={data?.prime_resurgence ?? null}
            now={now}
          />
          <MarketSalesPanel sales={data?.market_sales ?? []} now={now} />
          <NightwavePanel nightwave={data?.nightwave ?? null} now={now} />
        </div>
      </div>
    </Page>
  );
}
