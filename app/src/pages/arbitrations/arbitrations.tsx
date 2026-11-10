import { CalendarDays, History } from "lucide-react";
import { useState } from "react";
import { FilterGrid } from "@/components/filter-grid";
import { FilterSelect } from "@/components/filter-select";
import { OptionTabs } from "@/components/option-tabs";
import { Page, Section } from "@/components/page";
import { SearchInput } from "@/components/search-input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useAsyncData } from "@/hooks/use-async-data";
import { useFilters } from "@/hooks/use-filters";
import { useNow } from "@/hooks/use-now";
import { useTabParam } from "@/hooks/use-tab-param";
import {
  ARBITRATION_RELOAD_EVENTS,
  ARBITRATION_TIERS,
  type ArbitrationPersonalFilter,
  currentSlot,
  groupByDay,
  liveSlots,
  nextMatch,
  nextSlot,
  nodeValues,
  personalMatch,
  slotSearchText,
} from "@/lib/arbitrations";
import { api } from "@/lib/bridge";
import { type FilterOption, plainOptions } from "@/lib/filters";
import { counted, num } from "@/lib/format";
import { useSettings } from "@/stores/app-store";
import type {
  ArbitrationDashboard,
  ArbitrationForecastStatus,
  ArbitrationTier,
} from "@/types";
import { useArbitrationMarks } from "./marks";
import {
  FavouriteNodesPanel,
  ForecastNotice,
  ForecastSource,
  RemindersPanel,
} from "./panels";
import { ArbitrationRunDialog } from "./run-dialog";
import { ArbitrationLiveRunCard, ArbitrationRunHistory } from "./runs";
import { DayList, HeroCards, type SlotMarks } from "./schedule";

const ALL_DAYS = "all";

const PERSONAL: FilterOption<ArbitrationPersonalFilter>[] = [
  { value: "favourites", label: "Favourite nodes" },
  { value: "matches", label: "Matches my filters" },
  { value: "reminders", label: "Reminders" },
];

const TIERS: FilterOption<ArbitrationTier>[] = ARBITRATION_TIERS.map(
  (tier) => ({ value: tier, label: tier }),
);

const STATUS_NOTE: Record<ArbitrationForecastStatus, string | null> = {
  fresh: null,
  cached: "The forecast could not be refreshed. This is the saved copy.",
  stale:
    "The saved forecast is older than a day. Arbitration alerts are paused until it refreshes.",
  unavailable: "The forecast is not available. Merframe retries on its own.",
};

function statusNote(dashboard: ArbitrationDashboard | null): string | null {
  return dashboard ? STATUS_NOTE[dashboard.status] : null;
}

export function ArbitrationsPage() {
  const { data, error, loading } = useAsyncData(
    "arbitrations",
    api.arbitrations,
    ARBITRATION_RELOAD_EVENTS,
  );
  const now = useNow();
  const alertsOn = useSettings().alerts.arbitrations.enabled;
  const { busy, toggleFavourite, toggleReminder } = useArbitrationMarks();
  const { data: runs, error: runsError } = useAsyncData(
    "arbitration-runs",
    api.arbitrationRuns,
    ["arbitrationRunsUpdated"],
  );
  const [tab, setTab] = useTabParam<"forecast" | "history">("forecast");
  const [reportId, setReportId] = useState<string | null>(null);
  const report = runs?.find((run) => run.id === reportId) ?? null;
  const [search, setSearch] = useState("");
  const [day, setDay] = useState(ALL_DAYS);
  const filters = useFilters<{
    mission: string | null;
    planet: string | null;
    faction: string | null;
    tier: ArbitrationTier | null;
    personal: ArbitrationPersonalFilter | null;
  }>({
    mission: null,
    planet: null,
    faction: null,
    tier: null,
    personal: null,
  });

  const schedule = data?.schedule ?? null;
  const notice = statusNote(data);
  const slots = liveSlots(schedule, now);
  const current = currentSlot(slots, now);
  const next = nextSlot(slots, now);
  const match = nextMatch(schedule, now);
  const nodes = schedule?.nodes ?? [];
  const marks: SlotMarks = {
    busy,
    reminderAllowed: data?.notifications_available ?? false,
    onFavourite: toggleFavourite,
    onReminder: toggleReminder,
  };

  const query = search.trim().toLocaleLowerCase();
  const chosen = filters.values;
  const shown = slots.filter(
    (slot) =>
      (!query || slotSearchText(slot).includes(query)) &&
      (chosen.mission === null || slot.mission_type === chosen.mission) &&
      (chosen.planet === null || slot.planet === chosen.planet) &&
      (chosen.faction === null || slot.faction === chosen.faction) &&
      (chosen.tier === null || slot.tier === chosen.tier) &&
      personalMatch(slot, chosen.personal),
  );
  const days = groupByDay(shown, now);
  const dayTabs = [
    { value: ALL_DAYS, label: "All days" },
    ...groupByDay(slots, now).map((day) => ({
      value: day.key,
      label: day.label,
    })),
  ];
  const shownDays = days.filter(
    (entry) => day === ALL_DAYS || entry.key === day,
  );

  const clear = () => {
    setSearch("");
    setDay(ALL_DAYS);
    filters.clear();
  };

  const favouriteNodes = nodes.filter((node) =>
    slots.some((slot) => slot.favourite && slot.node_id === node.node_id),
  );
  const reminders = slots.filter(
    (slot) => slot.reminder && new Date(slot.starts).getTime() > now,
  );

  return (
    <Page
      title="Arbitrations"
      quote="arbitrations"
      loading={loading && data === null}
      error={error}
    >
      {notice && <ForecastNotice>{notice}</ForecastNotice>}
      <ArbitrationLiveRunCard now={now} />
      <HeroCards
        current={current}
        next={next}
        match={match}
        hasMatchRules={schedule?.has_match_rules ?? false}
        now={now}
        marks={marks}
      />

      <Tabs value={tab} onValueChange={setTab}>
        <TabsList className="mb-4 h-10">
          <TabsTrigger value="forecast" className="h-8 px-3 text-base">
            <CalendarDays className="size-4" />
            Forecast
          </TabsTrigger>
          <TabsTrigger value="history" className="h-8 px-3 text-base">
            <History className="size-4" />
            Run History ({num(runs?.length ?? 0)})
          </TabsTrigger>
        </TabsList>
        <TabsContent value="forecast" className="flex flex-col gap-6">
          <div className="grid gap-3 @lg:grid-cols-2">
            <FavouriteNodesPanel
              nodes={favouriteNodes}
              busy={busy}
              onRemove={toggleFavourite}
            />
            <RemindersPanel
              slots={reminders}
              alertsOn={alertsOn}
              busy={busy}
              onRemove={toggleReminder}
            />
          </div>

          <Section
            className="min-w-0"
            title="Next Seven Days"
            description={counted(shown.length, "rotation")}
            action={
              <OptionTabs
                value={
                  dayTabs.some((tab) => tab.value === day) ? day : ALL_DAYS
                }
                options={dayTabs}
                onChange={setDay}
              />
            }
          >
            <div className="flex flex-col gap-4">
              <SearchInput
                value={search}
                onValueChange={setSearch}
                placeholder="Search the schedule"
              />
              <FilterGrid
                activeFilters={filters.active}
                onClear={filters.clear}
              >
                <FilterSelect
                  label="Mission type"
                  value={chosen.mission}
                  options={plainOptions(nodeValues(nodes, "mission_type"))}
                  onChange={(value) => filters.set("mission", value)}
                />
                <FilterSelect
                  label="Planet"
                  value={chosen.planet}
                  options={plainOptions(nodeValues(nodes, "planet"))}
                  onChange={(value) => filters.set("planet", value)}
                />
                <FilterSelect
                  label="Faction"
                  value={chosen.faction}
                  options={plainOptions(nodeValues(nodes, "faction"))}
                  onChange={(value) => filters.set("faction", value)}
                />
                <FilterSelect
                  label="Community tier"
                  value={chosen.tier}
                  options={TIERS}
                  onChange={(value) => filters.set("tier", value)}
                />
                <FilterSelect
                  label="Personal"
                  value={chosen.personal}
                  options={PERSONAL}
                  onChange={(value) => filters.set("personal", value)}
                />
              </FilterGrid>
              <DayList
                days={shownDays}
                total={slots.length}
                now={now}
                marks={marks}
                onClear={clear}
              />
            </div>
          </Section>

          <ForecastSource />
        </TabsContent>
        <TabsContent value="history">
          <ArbitrationRunHistory
            runs={runs}
            error={runsError}
            onOpenReport={(run) => setReportId(run.id)}
          />
        </TabsContent>
      </Tabs>
      {report && (
        <ArbitrationRunDialog run={report} onClose={() => setReportId(null)} />
      )}
    </Page>
  );
}
