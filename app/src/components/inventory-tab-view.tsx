import { ArrowDownWideNarrow, ArrowUpNarrowWide } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Amount } from "@/components/currency";
import { FilterGrid } from "@/components/filter-grid";
import { FilterSelect } from "@/components/filter-select";
import { ItemCard } from "@/components/inventory-item-card";
import { prefetchImages } from "@/components/item-image";
import { OptionSelect } from "@/components/option-select";
import { CardGrid, EmptyPanel } from "@/components/page";
import { Pagination } from "@/components/pagination";
import { SearchInput } from "@/components/search-input";
import { Button } from "@/components/ui/button";
import { usePaged } from "@/hooks/use-paged";
import {
  NO_FILTER_MATCHES,
  plainOptions,
  withYesNo,
  yesNoOptions,
} from "@/lib/filters";
import { num } from "@/lib/format";
import {
  ducats,
  entryKey,
  type InventoryEntry,
  type InventoryFlags,
  totalPlat,
} from "@/lib/inventory-entries";
import {
  descending,
  type InventoryTabKey,
  keeps,
  loadFilters,
  ORDERINGS,
  refinementsFor,
  saveFilters,
  sortEntries,
  type TabFilters,
  yesNoFiltersFor,
} from "@/lib/inventory-filters";

const MIN_PLAT_CHOICES = plainOptions(["5", "10", "15"]);

export function InventoryTabView({
  tab,
  entries,
  flags,
}: {
  tab: InventoryTabKey;
  entries: InventoryEntry[];
  flags: InventoryFlags;
}) {
  const [filters, setFilters] = useState<TabFilters>(() => loadFilters(tab));

  const update = (patch: Partial<TabFilters>) => {
    setFilters((current) => ({ ...current, ...patch }));
  };

  useEffect(() => {
    saveFilters(tab, filters);
  }, [tab, filters]);

  const desc = descending(filters);
  const sorted = useMemo(
    () => sortEntries(entries, filters.ordering, desc, tab),
    [entries, filters.ordering, desc, tab],
  );

  const visible = useMemo(
    () => sorted.filter((entry) => keeps(entry, filters, flags)),
    [sorted, filters, flags],
  );

  const { pageItems: pageEntries, pagination } = usePaged(
    visible,
    JSON.stringify(filters),
  );

  const selection = useMemo(
    () =>
      visible.reduce(
        (accumulator, entry) => ({
          ducats: accumulator.ducats + (ducats(entry) ?? 0) * entry.row.count,
          plat: accumulator.plat + (totalPlat(entry) ?? 0) * entry.row.count,
          priced: accumulator.priced + (totalPlat(entry) === null ? 0 : 1),
        }),
        { ducats: 0, plat: 0, priced: 0 },
      ),
    [visible],
  );

  useEffect(() => {
    prefetchImages([
      ...pageEntries.map((entry) => entry.item.image_name),
      ...pageEntries.flatMap((entry) =>
        entry.kind === "set"
          ? entry.parts.map((part) => part.item.image_name)
          : [],
      ),
    ]);
  }, [pageEntries]);

  const refinements = refinementsFor(tab);
  const activeFilters =
    Object.keys(filters.yesNo).length +
    (filters.minPlat === null ? 0 : 1) +
    (filters.refinement === null ? 0 : 1);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <SearchInput
          value={filters.search}
          onValueChange={(search) => update({ search })}
          placeholder="Search"
        />

        <OptionSelect
          label="Order by"
          value={filters.ordering}
          options={ORDERINGS}
          onChange={(ordering) => update({ ordering })}
          className="w-44"
        />

        <Button
          variant="outline"
          size="icon"
          aria-label={descending(filters) ? "Largest first" : "Smallest first"}
          title={descending(filters) ? "Largest first" : "Smallest first"}
          onClick={() => update({ preferHighest: !filters.preferHighest })}
        >
          {descending(filters) ? (
            <ArrowDownWideNarrow className="size-4" />
          ) : (
            <ArrowUpNarrowWide className="size-4" />
          )}
        </Button>

        <span className="text-muted-foreground ml-auto text-xs">
          {num(visible.length)} of {num(entries.length)} shown
          {selection.priced < visible.length
            ? `, ${num(visible.length - selection.priced)} without a price yet`
            : ""}
        </span>
      </div>

      <FilterGrid
        activeFilters={activeFilters}
        onClear={() => update({ yesNo: {}, minPlat: null, refinement: null })}
      >
        {yesNoFiltersFor(tab).map((group) => (
          <FilterSelect
            key={group.key}
            label={group.label}
            value={filters.yesNo[group.key] ?? null}
            options={yesNoOptions(group.yes, group.no)}
            onChange={(mode) =>
              update({
                yesNo: withYesNo(filters.yesNo, group.key, mode),
              })
            }
          />
        ))}
        {refinements.length > 0 && (
          <FilterSelect
            label="Refinement"
            value={filters.refinement}
            options={plainOptions(refinements)}
            onChange={(value) =>
              update({
                refinement: value,
              })
            }
          />
        )}
        <FilterSelect
          label="Min plat"
          value={filters.minPlat === null ? null : String(filters.minPlat)}
          options={MIN_PLAT_CHOICES}
          onChange={(value) =>
            update({ minPlat: value === null ? null : Number(value) })
          }
        />
      </FilterGrid>

      {pageEntries.length === 0 ? (
        <EmptyPanel>{NO_FILTER_MATCHES}</EmptyPanel>
      ) : (
        <CardGrid className="@sm:grid-cols-1 @2xl:grid-cols-2 @6xl:grid-cols-3 @7xl:grid-cols-3">
          {pageEntries.map((entry) => (
            <ItemCard
              key={entryKey(entry)}
              entry={entry}
              tab={tab}
              flags={flags}
            />
          ))}
        </CardGrid>
      )}

      <Pagination {...pagination} />

      <div className="text-muted-foreground flex items-center justify-end gap-6 text-sm">
        <span
          className="flex items-center gap-1"
          title="Total of the rows shown"
        >
          Ducats
          <Amount currency="ducats" className="text-foreground font-semibold">
            {num(selection.ducats)}
          </Amount>
        </span>
        <span
          className="flex items-center gap-1"
          title="Total of the rows shown"
        >
          Platinum
          <Amount currency="platinum" className="font-semibold">
            {num(selection.plat)}
          </Amount>
        </span>
      </div>
    </div>
  );
}
