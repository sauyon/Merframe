import type { HeldResource, ResourceRow } from "@/types";

export type ShowFilter = "all" | "short";

export function visibleResources(
  resources: ResourceRow[],
  show: ShowFilter,
  search: string,
): ResourceRow[] {
  const needle = search.trim().toLowerCase();
  return resources
    .filter(
      (row) =>
        (show === "all" || row.deficit > 0) &&
        row.name.toLowerCase().includes(needle),
    )
    .sort((left, right) =>
      left.deficit === right.deficit
        ? left.name.localeCompare(right.name)
        : right.deficit - left.deficit,
    );
}

export type HoldingsView = "held" | "currencies";

export function visibleHoldings(
  holdings: HeldResource[],
  view: HoldingsView,
  search: string,
): HeldResource[] {
  const needle = search.trim().toLowerCase();
  return holdings
    .filter(
      (row) =>
        row.owned > 0 &&
        (view === "held" || row.vendors.length > 0) &&
        row.name.toLowerCase().includes(needle),
    )
    .sort((left, right) => left.name.localeCompare(right.name));
}
