import { Filter, X } from "lucide-react";
import { type RefObject, useCallback, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { useShallow } from "zustand/react/shallow";
import { DataTable } from "@/components/data-table";
import { FilterField, FilterGrid } from "@/components/filter-grid";
import { FilterSelect, FilterSingleSelect } from "@/components/filter-select";
import { OptionTabs } from "@/components/option-tabs";
import { Page, Section } from "@/components/page";
import { Button } from "@/components/ui/button";
import { CheckboxField } from "@/components/ui/checkbox";
import { useAsyncData } from "@/hooks/use-async-data";
import { useFilters } from "@/hooks/use-filters";
import { api } from "@/lib/bridge";
import {
  ANY,
  type FilterOption,
  plainOptions,
  yesNoOptions,
} from "@/lib/filters";
import { counted, num, plat } from "@/lib/format";
import {
  type FavouriteFilter,
  matchesRelicFilters,
  type RelicFilters,
  refinementValue,
  showsWanted,
  writeOverlayFilters,
} from "@/lib/relic-filters";
import { REFINEMENTS, RELIC_TIERS } from "@/lib/relics";
import { notify } from "@/lib/toast";
import { usePreferencesStore } from "@/stores/preferences-store";
import type { MissingPart } from "@/types";
import {
  type PlannerRow,
  plannerColumns,
  rewardViews,
  searchValue,
} from "./columns";
import { PlannerRowDetails } from "./details";
import { PartSources, WantedParts } from "./parts";

const SQUAD_SIZES = ["1", "2", "3", "4"] as const;
const FAVOURITE_OPTIONS: FilterOption<FavouriteFilter>[] = [
  { value: "yes", label: "Favourites" },
  { value: "no", label: "Not favourites" },
  { value: "order", label: "Order first" },
];
const ALL_ERAS = "all";

type Choices = Pick<
  RelicFilters,
  | "vaulted"
  | "itemsOwned"
  | "setsOwned"
  | "stacked"
  | "refined"
  | "tierOwned"
  | "favourite"
>;

export function RelicPlannerPage() {
  const { squadSize, refinement, era, setSquadSize, setRefinement, setEra } =
    usePreferencesStore(
      useShallow((state) => ({
        squadSize: state.squadSize,
        refinement: state.refinement,
        era: state.era,
        setSquadSize: state.setSquadSize,
        setRefinement: state.setRefinement,
        setEra: state.setEra,
      })),
    );
  const [onlyOwned, setOnlyOwned] = useState(true);
  const choices = useFilters<Choices>({
    vaulted: null,
    itemsOwned: null,
    setsOwned: null,
    stacked: null,
    refined: null,
    tierOwned: null,
    favourite: null,
  });
  const {
    vaulted,
    itemsOwned,
    setsOwned,
    stacked,
    refined,
    tierOwned,
    favourite,
  } = choices.values;
  const [wanted, setWanted] = useState<MissingPart[]>([]);
  const [selectedPart, setSelectedPart] = useState<MissingPart | null>(null);
  const wantedPanel = useRef<HTMLDivElement>(null);
  const sourcesPanel = useRef<HTMLDivElement>(null);
  const [params] = useSearchParams();

  const wantedPaths = useMemo(
    () => wanted.map((part) => part.unique_name),
    [wanted],
  );

  const load = useCallback(
    () => api.relicPlannerTab(squadSize, onlyOwned, wantedPaths),
    [squadSize, onlyOwned, wantedPaths],
  );

  const { data, error, loading } = useAsyncData(
    `relic_planner_tab:${JSON.stringify([squadSize, onlyOwned, wantedPaths])}`,
    load,
    ["inventoryUpdated", "pricesUpdated", "favouriteUpdated"],
  );

  const scrollToPanel = (panel: RefObject<HTMLDivElement | null>) => {
    panel.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  const missing = data?.missing_parts ?? [];

  const filters = useMemo<RelicFilters>(
    () => ({
      squadSize,
      refinement,
      ...choices.values,
      wanted: wantedPaths,
    }),
    [squadSize, refinement, choices.values, wantedPaths],
  );

  const handlePushToOverlay = useCallback(() => {
    writeOverlayFilters(filters);
    notify.success("The relic overlay now uses these filters");
  }, [filters]);

  const owned = data?.plans ?? [];

  const eras = useMemo(() => {
    const present = new Set(owned.map((plan) => plan.tier));
    return [
      { value: ALL_ERAS, label: "All" },
      ...plainOptions(RELIC_TIERS.filter((tier) => present.has(tier))),
    ];
  }, [owned]);

  const rows = useMemo(
    () =>
      owned
        .filter(
          (plan) =>
            (era === ALL_ERAS || plan.tier === era) &&
            matchesRelicFilters(plan, filters),
        )
        .flatMap<PlannerRow>((plan) => {
          const best = refinement === ANY;
          const value = refinementValue(plan, refinement);
          if (!value) {
            return [];
          }
          return [
            {
              plan,
              value,
              rewards: rewardViews(plan, value),
              perTrace: best
                ? (plan.best.plat_per_trace?.value ?? null)
                : value.plat_per_trace,
              perTraceRefinement: best
                ? (plan.best.plat_per_trace?.refinement ?? null)
                : value.refinement,
              ducatsPerTrace: best
                ? (plan.best.ducats_per_trace?.value ?? null)
                : value.ducats_per_trace,
              ducatTraceRefinement: best
                ? (plan.best.ducats_per_trace?.refinement ?? null)
                : value.refinement,
            },
          ];
        }),
    [owned, era, filters, refinement],
  );

  const best = useMemo(
    () =>
      rows.reduce<PlannerRow | null>(
        (top, row) =>
          top === null || row.value.expected_plat > top.value.expected_plat
            ? row
            : top,
        null,
      ),
    [rows],
  );

  const columns = useMemo(
    () => plannerColumns(squadSize, showsWanted(owned)),
    [squadSize, owned],
  );

  const traces = data?.void_traces ?? 0;
  const summary = best
    ? `${counted(rows.length, "relic")}, best is ${best.plan.relic} at ${plat(best.value.expected_plat)} platinum expected (${best.value.refinement}), ${num(traces)} Void Traces`
    : `No relic matches these filters, ${num(traces)} Void Traces`;

  return (
    <Page
      title="Relic Planner"
      quote="relicPlanner"
      actions={
        <>
          <Button variant="outline" size="sm" onClick={handlePushToOverlay}>
            Push Filters to the Overlay
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => scrollToPanel(wantedPanel)}
          >
            Wanted Parts
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => scrollToPanel(sourcesPanel)}
          >
            Drops From
          </Button>
        </>
      }
      loading={loading && !data}
      error={error}
    >
      <Section
        title={onlyOwned ? "Owned Relics" : "Every Relic"}
        description={summary}
        action={<OptionTabs value={era} options={eras} onChange={setEra} />}
      >
        <DataTable
          tableId="relicPlanner"
          columns={columns}
          data={rows}
          resetKey={JSON.stringify([onlyOwned, era, filters])}
          searchPlaceholder="Filter relics and rewards"
          searchValue={searchValue}
          initialSearch={params.get("search") ?? ""}
          initialSorting={[{ id: "expected_plat", desc: true }]}
          primarySort={
            favourite === "order"
              ? [{ id: "favourites", desc: true }]
              : undefined
          }
          rowKey={(row) => row.plan.relic}
          toolbar={
            wanted.length > 0 && (
              <Button variant="outline" size="sm" onClick={() => setWanted([])}>
                <Filter className="size-3.5" />
                {counted(wanted.length, "wanted part")}
                <X className="size-3.5" />
              </Button>
            )
          }
          filters={
            <FilterGrid activeFilters={choices.active} onClear={choices.clear}>
              <FilterSingleSelect
                label="Squad"
                value={String(squadSize)}
                options={plainOptions(SQUAD_SIZES)}
                onChange={(size) => setSquadSize(Number(size))}
              />
              <FilterSingleSelect
                label="Refinement"
                value={refinement}
                options={[
                  { value: ANY, label: "Any" },
                  ...plainOptions(REFINEMENTS),
                ]}
                onChange={setRefinement}
              />
              <FilterSelect
                label="Vault"
                value={vaulted}
                options={yesNoOptions("Vaulted", "Available")}
                onChange={(value) => choices.set("vaulted", value)}
              />
              <FilterSelect
                label="Items owned"
                value={itemsOwned}
                options={yesNoOptions("All built or mastered", "Some missing")}
                onChange={(value) => choices.set("itemsOwned", value)}
              />
              <FilterSelect
                label="Sets owned"
                value={setsOwned}
                options={yesNoOptions("All complete", "Incomplete")}
                onChange={(value) => choices.set("setsOwned", value)}
              />
              <FilterSelect
                label="Copies"
                value={stacked}
                options={yesNoOptions("10 or more", "Fewer than 10")}
                onChange={(value) => choices.set("stacked", value)}
              />
              <FilterSelect
                label="Refined"
                value={refined}
                options={yesNoOptions("Refined copies", "Intact only")}
                onChange={(value) => choices.set("refined", value)}
              />
              <FilterSelect
                label="Tier owned"
                value={tierOwned}
                options={plainOptions(REFINEMENTS)}
                onChange={(value) => choices.set("tierOwned", value)}
              />
              <FilterSelect
                label="Favourite"
                value={favourite}
                options={FAVOURITE_OPTIONS}
                onChange={(value) => choices.set("favourite", value)}
              />
              <FilterField label="Relics">
                <CheckboxField
                  id="onlyOwnedRelics"
                  checked={onlyOwned}
                  onChange={setOnlyOwned}
                  className="h-8"
                >
                  Only owned
                </CheckboxField>
              </FilterField>
            </FilterGrid>
          }
          renderSubRow={(row) => <PlannerRowDetails row={row} />}
        />
      </Section>

      <div className="grid gap-4 @3xl:grid-cols-2">
        <div ref={wantedPanel} className="scroll-mt-6">
          <WantedParts
            missing={missing}
            wanted={wanted}
            setWanted={setWanted}
            selected={selectedPart}
            onSelect={setSelectedPart}
          />
        </div>

        <div ref={sourcesPanel} className="scroll-mt-6">
          <PartSources part={selectedPart} />
        </div>
      </div>
    </Page>
  );
}
