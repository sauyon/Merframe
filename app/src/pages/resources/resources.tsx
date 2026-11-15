import { useCallback, useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { Amount } from "@/components/currency";
import { FilterGrid } from "@/components/filter-grid";
import { FilterSelect, FilterSingleSelect } from "@/components/filter-select";
import { prefetchImages } from "@/components/item-image";
import {
  CardGrid,
  EmptyPanel,
  Page,
  ResultCount,
  Section,
  Stat,
} from "@/components/page";
import { Pagination } from "@/components/pagination";
import { SearchInput } from "@/components/search-input";
import { Spinner } from "@/components/ui/spinner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useAsyncData } from "@/hooks/use-async-data";
import { useFilters } from "@/hooks/use-filters";
import { usePaged } from "@/hooks/use-paged";
import { useTabParam } from "@/hooks/use-tab-param";
import { api } from "@/lib/bridge";
import { NO_FILTER_MATCHES, type YesNo, yesNoOptions } from "@/lib/filters";
import { num } from "@/lib/format";
import { CATEGORIES } from "@/lib/foundry-filters";
import {
  type HoldingsView,
  type ShowFilter,
  visibleHoldings,
  visibleResources,
} from "@/lib/resource-filters";
import { cn } from "@/lib/utils";
import { FoundryTreeDialog } from "@/pages/foundry/tree-dialog";
import { usePreferencesStore } from "@/stores/preferences-store";
import type { ResourceScope, ResourceSource } from "@/types";
import { ArchonShards } from "./archon-shards";
import { Holdings } from "./holdings";
import { NeededBy } from "./needed-by";
import { ResourceCard } from "./resource-card";

const SOURCES: readonly { value: ResourceSource; label: string }[] = [
  { value: "held", label: "With a held blueprint" },
  { value: "craftable", label: "All craftable" },
];

const SCOPES: readonly { value: ResourceScope; label: string }[] = [
  { value: "mastery", label: "Missing for mastery" },
  { value: "all", label: "All items" },
  { value: "starred", label: "Starred" },
];

const SHOWN: readonly { value: ShowFilter; label: string }[] = [
  { value: "all", label: "All resources" },
  { value: "short", label: "Only shortfalls" },
];

const KINDS = CATEGORIES.filter(([key]) => key !== "all").map(
  ([value, label]) => ({ value, label }),
);
const PRIME = yesNoOptions("Prime", "Normal");
const OWNED = yesNoOptions("Yes", "No");

interface ResourceFilters {
  kind: string | null;
  prime: YesNo | null;
  owned: YesNo | null;
}

const NO_DEMAND: Record<ResourceSource, Record<ResourceScope, string>> = {
  held: {
    mastery:
      "No blueprint in the inventory belongs to an item that is neither owned nor mastered.",
    all: "The inventory holds no blueprint with a resource cost.",
    starred: "No blueprint in the inventory is starred.",
  },
  craftable: {
    mastery: "Every craftable item is already owned or mastered.",
    all: "No craftable item has a resource cost.",
    starred: "No craftable item is starred.",
  },
};

type ResourcesTabKey = "needed" | HoldingsView;

export function ResourcesPage() {
  const [tab, setTab] = useTabParam<ResourcesTabKey>("needed");
  const { source, scope, setResourceSource, setResourceScope } =
    usePreferencesStore(
      useShallow((state) => ({
        source: state.resourceSource,
        scope: state.resourceScope,
        setResourceSource: state.setResourceSource,
        setResourceScope: state.setResourceScope,
      })),
    );
  const filters = useFilters<ResourceFilters>({
    kind: null,
    prime: null,
    owned: null,
  });
  const { kind, prime, owned } = filters.values;
  const [show, setShow] = useState<ShowFilter>("all");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [opened, setOpened] = useState<string | null>(null);

  const load = useCallback(
    () =>
      api.resourcesTab({
        source,
        scope,
        kind,
        prime: prime === null ? null : prime === "yes",
        owned: owned === null ? null : owned === "yes",
      }),
    [source, scope, kind, prime, owned],
  );
  const { data, error, loading } = useAsyncData(
    `resources_tab:${JSON.stringify([source, scope, kind, prime, owned])}`,
    load,
    ["inventoryUpdated", "favouriteUpdated"],
  );

  const resources = useMemo(
    () => visibleResources(data?.resources ?? [], show, query),
    [data?.resources, show, query],
  );

  const { pageItems: pageRows, pagination } = usePaged(
    resources,
    JSON.stringify([source, scope, kind, prime, owned, show, query]),
  );

  const selectedRow = useMemo(
    () =>
      pageRows.find((row) => row.unique_name === selected) ??
      pageRows[0] ??
      null,
    [pageRows, selected],
  );

  const openedItem = useMemo(
    () =>
      data?.resources
        .flatMap((row) => row.used_by)
        .find((use) => use.unique_name === opened) ?? null,
    [data?.resources, opened],
  );
  const handleCloseTree = useCallback(() => setOpened(null), []);

  useEffect(() => {
    prefetchImages([
      ...pageRows.map((row) => row.image_name),
      ...(selectedRow?.used_by.map((use) => use.image_name) ?? []),
      ...(data?.shards.map((shard) => shard.image_name) ?? []),
    ]);
  }, [pageRows, selectedRow, data?.shards]);

  const short = data?.resources.filter((row) => row.deficit > 0).length ?? 0;
  const holdings = data?.holdings ?? [];
  const heldCount = useMemo(
    () => visibleHoldings(holdings, "held", "").length,
    [holdings],
  );
  const currencyCount = useMemo(
    () => visibleHoldings(holdings, "currencies", "").length,
    [holdings],
  );

  return (
    <Page
      title="Resources"
      quote="resources"
      loading={loading && !data}
      error={error}
    >
      {data && data.shards.length > 0 && <ArchonShards shards={data.shards} />}

      <Tabs value={tab} onValueChange={setTab}>
        <TabsList>
          <TabsTrigger value="needed">Needed ({num(short)} short)</TabsTrigger>
          <TabsTrigger value="held">All Held ({num(heldCount)})</TabsTrigger>
          <TabsTrigger value="currencies">
            Vendor Currencies ({num(currencyCount)})
          </TabsTrigger>
        </TabsList>

        <TabsContent value="held">
          <Holdings holdings={holdings} view="held" loading={loading} />
        </TabsContent>
        <TabsContent value="currencies">
          <Holdings holdings={holdings} view="currencies" loading={loading} />
        </TabsContent>

        <TabsContent value="needed" className="flex flex-col gap-4">
          <div className="grid gap-4 @sm:grid-cols-3">
            <Stat label="Short resources" value={num(short)} />
            <Stat label="Items to build" value={num(data?.items)} />
            <Stat
              label="Credits needed"
              value={
                <Amount currency="credits" size={20}>
                  {num(data?.credits)}
                </Amount>
              }
            />
          </div>

          <div className="grid gap-4 @3xl:grid-cols-3">
            <Section
              title="Stockpile"
              className={selectedRow ? "@3xl:col-span-2" : "@3xl:col-span-3"}
              action={
                <div className="flex items-center gap-3">
                  {loading && (
                    <Spinner
                      role="status"
                      aria-label="Loading"
                      className="text-muted-foreground"
                    />
                  )}
                  <ResultCount
                    shown={resources.length}
                    total={data?.resources.length}
                    noun="resource"
                  />
                  <SearchInput
                    value={query}
                    onValueChange={setQuery}
                    placeholder="Search"
                  />
                </div>
              }
            >
              <div className="flex flex-col gap-3">
                <FilterGrid className="border-0 p-0">
                  <FilterSingleSelect
                    label="Items"
                    value={source}
                    options={SOURCES}
                    onChange={setResourceSource}
                  />
                  <FilterSingleSelect
                    label="Demand from"
                    value={scope}
                    options={SCOPES}
                    onChange={setResourceScope}
                  />
                  <FilterSingleSelect
                    label="Show"
                    value={show}
                    options={SHOWN}
                    onChange={setShow}
                  />
                </FilterGrid>

                <FilterGrid
                  className="border-t pt-3"
                  activeFilters={filters.active}
                  onClear={filters.clear}
                >
                  <FilterSelect
                    label="Category"
                    value={kind}
                    options={KINDS}
                    onChange={(value) => filters.set("kind", value)}
                  />
                  <FilterSelect
                    label="Type"
                    value={prime}
                    options={PRIME}
                    onChange={(value) => filters.set("prime", value)}
                  />
                  <FilterSelect
                    label="Owned"
                    value={owned}
                    options={OWNED}
                    onChange={(value) => filters.set("owned", value)}
                  />
                </FilterGrid>

                {resources.length === 0 ? (
                  <EmptyPanel>
                    {data?.resources.length === 0 && filters.active === 0
                      ? NO_DEMAND[source][scope]
                      : NO_FILTER_MATCHES}
                  </EmptyPanel>
                ) : (
                  <CardGrid
                    className={cn(
                      "transition-opacity @5xl:grid-cols-2 @7xl:grid-cols-3",
                      loading && "opacity-60",
                    )}
                  >
                    {pageRows.map((row) => (
                      <ResourceCard
                        key={row.unique_name}
                        row={row}
                        selected={row === selectedRow}
                        onSelect={setSelected}
                      />
                    ))}
                  </CardGrid>
                )}

                <Pagination {...pagination} />
              </div>
            </Section>

            {selectedRow && <NeededBy row={selectedRow} onOpen={setOpened} />}
          </div>
        </TabsContent>
      </Tabs>

      <FoundryTreeDialog item={openedItem} onClose={handleCloseTree} />
    </Page>
  );
}
