import type { ColumnDef } from "@tanstack/react-table";
import { TrendingDown, TrendingUp } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import {
  ChangeBars,
  type ChangeRow,
  signedPercent,
} from "@/components/change-bars";
import { DataTable } from "@/components/data-table";
import { FilterGrid } from "@/components/filter-grid";
import { FilterSelect } from "@/components/filter-select";
import { OptionTabs } from "@/components/option-tabs";
import { ErrorNote, Section, TableSkeleton } from "@/components/page";
import { PieChart } from "@/components/pie-chart";
import { Badge } from "@/components/ui/badge";
import { SwitchField } from "@/components/ui/switch";
import { useAsyncData } from "@/hooks/use-async-data";
import { useFilters } from "@/hooks/use-filters";
import { median } from "@/lib/arrays";
import { api } from "@/lib/bridge";
import { changeTone } from "@/lib/chart";
import { num, plat } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { MarketMover, MarketWindow } from "@/types";
import {
  CATEGORY_LABELS,
  categorySlices,
  TRADE_CATEGORIES,
} from "./categories";
import { itemColumn, totalValueColumn } from "./columns";

const WINDOWS: { value: MarketWindow; label: string }[] = [
  { value: "2", label: "48 hours" },
  { value: "7", label: "7 days" },
  { value: "30", label: "30 days" },
  { value: "90", label: "90 days" },
];

type Measure = "value" | "volume";

const MEASURES: { value: Measure; label: string }[] = [
  { value: "value", label: "Platinum" },
  { value: "volume", label: "Amount" },
];

const MEASURE_TITLES: Record<Measure, string> = {
  value: "Platinum traded by category",
  volume: "Amount traded by category",
};

const CHANGE_TITLES: Record<Measure, string> = {
  value: "Platinum vs previous window by category",
  volume: "Amount vs previous window by category",
};

function valueChange(mover: MarketMover): number | null {
  return mover.price_change === null || mover.volume_change === null
    ? null
    : (1 + mover.price_change) * (1 + mover.volume_change) - 1;
}

const PAGE_SIZE = 10;

interface MoverFilters {
  category: string | null;
  price: string | null;
  volume: string | null;
}

const CATEGORY_OPTIONS = Object.entries(CATEGORY_LABELS).map(
  ([value, label]) => ({ value, label }),
);

const PRICE_BANDS: Record<
  string,
  { label: string; low: number; high: number }
> = {
  under10: { label: "Under 10", low: 0, high: 10 },
  under50: { label: "10 to 49", low: 10, high: 50 },
  under200: { label: "50 to 199", low: 50, high: 200 },
  over200: { label: "200 and over", low: 200, high: Number.POSITIVE_INFINITY },
};

const PRICE_OPTIONS = Object.entries(PRICE_BANDS).map(([value, band]) => ({
  value,
  label: band.label,
}));

const VOLUME_OPTIONS = [10, 100, 1000].map((least) => ({
  value: String(least),
  label: `${num(least)} or more`,
}));

function Change({ change }: { change: number | null }) {
  if (change === null) {
    return "-";
  }
  const percent = Math.round(change * 100);
  if (percent === 0) {
    return "0%";
  }
  const Icon = percent > 0 ? TrendingUp : TrendingDown;
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 font-medium",
        changeTone(percent),
      )}
    >
      <Icon className="size-4" aria-hidden="true" />
      {signedPercent(change)}
    </span>
  );
}

const COMPARISON_COLUMNS: ColumnDef<MarketMover>[] = [
  {
    accessorKey: "price_change",
    header: "Price vs previous",
    meta: { numeric: true },
    cell: ({ row }) => <Change change={row.original.price_change} />,
  },
  {
    accessorKey: "volume_change",
    header: "Volume vs previous",
    meta: { numeric: true },
    cell: ({ row }) => <Change change={row.original.volume_change} />,
  },
  {
    id: "value_change",
    header: "Value vs previous",
    meta: { numeric: true },
    accessorFn: valueChange,
    cell: ({ getValue }) => <Change change={getValue<number | null>()} />,
  },
];

const COLUMNS: ColumnDef<MarketMover>[] = [
  itemColumn(),
  {
    id: "category",
    header: "Category",
    accessorFn: (row) => CATEGORY_LABELS[row.category],
    cell: ({ row }) => (
      <Badge variant="muted">{CATEGORY_LABELS[row.original.category]}</Badge>
    ),
  },
  {
    accessorKey: "unit_price",
    header: "Unit price",
    meta: { numeric: true },
    cell: ({ row }) =>
      Number.isInteger(row.original.unit_price)
        ? num(row.original.unit_price)
        : plat(row.original.unit_price),
  },
  {
    accessorKey: "volume",
    header: "Volume",
    meta: { numeric: true },
    cell: ({ row }) => num(row.original.volume),
  },
  totalValueColumn(),
];

export function MarketMovers() {
  const [range, setRange] = useState<MarketWindow>("7");
  const [measure, setMeasure] = useState<Measure>("value");
  const filters = useFilters<MoverFilters>({
    category: null,
    price: null,
    volume: null,
  });
  const { category, price, volume } = filters.values;
  const [compare, setCompare] = useState(false);
  const comparable = range !== "90";
  const compared = compare && comparable;
  const columns = useMemo(
    () => (compared ? [...COLUMNS, ...COMPARISON_COLUMNS] : COLUMNS),
    [compared],
  );
  const load = useCallback(() => api.marketMovers(range), [range]);
  const { data, error, loading } = useAsyncData(
    `market_movers:${range}`,
    load,
    ["pricesUpdated"],
  );

  const slices = useMemo(
    () =>
      categorySlices((category) =>
        (data ?? [])
          .filter((mover) => mover.category === category)
          .reduce((sum, mover) => sum + mover[measure], 0),
      ),
    [data, measure],
  );

  const changes = useMemo<ChangeRow[]>(
    () =>
      TRADE_CATEGORIES.flatMap((category) => {
        const known = (data ?? [])
          .filter((mover) => mover.category === category)
          .map((mover) =>
            measure === "value" ? valueChange(mover) : mover.volume_change,
          )
          .filter((change): change is number => change !== null);
        return known.length === 0
          ? []
          : [
              {
                key: category,
                label: CATEGORY_LABELS[category],
                change: median(known),
                note: `median of ${num(known.length)} items`,
              },
            ];
      }).sort((a, b) => b.change - a.change),
    [data, measure],
  );

  const rows = useMemo(() => {
    const band = price === null ? null : PRICE_BANDS[price];
    const least = volume === null ? 0 : Number(volume);
    return (data ?? []).filter(
      (mover) =>
        (category === null || mover.category === category) &&
        (!band ||
          (mover.unit_price >= band.low && mover.unit_price < band.high)) &&
        mover.volume >= least,
    );
  }, [data, category, price, volume]);

  if (loading && !data) {
    return <TableSkeleton />;
  }

  return (
    <Section
      title="Top Market Items"
      description={
        compared
          ? "Unit price, volume and total value of the window against the window of the same length right before it."
          : "What changed hands on warframe.market in the window. Unit price is the median sale, total value is unit price times volume."
      }
      action={
        <div className="flex flex-wrap items-center gap-4">
          <SwitchField
            id="compare"
            label="Compare to previous"
            checked={compared}
            disabled={!comparable}
            onChange={setCompare}
          />
          <OptionTabs value={range} options={WINDOWS} onChange={setRange} />
        </div>
      }
    >
      {error && <ErrorNote message={error} />}
      <div
        className={cn(
          "flex flex-col gap-6 transition-opacity",
          loading && "opacity-60",
        )}
      >
        <DataTable
          tableId="analyticsMovers"
          columns={columns}
          data={rows}
          resetKey={JSON.stringify([range, measure, category, price, volume])}
          filters={
            <FilterGrid activeFilters={filters.active} onClear={filters.clear}>
              <FilterSelect
                label="Category"
                value={category}
                options={CATEGORY_OPTIONS}
                onChange={(value) => filters.set("category", value)}
              />
              <FilterSelect
                label="Unit price"
                value={price}
                options={PRICE_OPTIONS}
                onChange={(value) => filters.set("price", value)}
              />
              <FilterSelect
                label="Volume"
                value={volume}
                options={VOLUME_OPTIONS}
                onChange={(value) => filters.set("volume", value)}
              />
            </FilterGrid>
          }
          searchPlaceholder="Filter items"
          initialSorting={[{ id: "value", desc: true }]}
          rowKey={(row) => row.slug}
          fixedPageSize={PAGE_SIZE}
          emptyMessage="No item sold in this window matches the filters."
        />
        <div className="grid gap-6 @3xl:grid-cols-2">
          <PieChart
            label={MEASURE_TITLES[measure]}
            slices={slices}
            format={num}
            action={
              <OptionTabs
                value={measure}
                options={MEASURES}
                onChange={setMeasure}
              />
            }
          />
          <ChangeBars
            label={CHANGE_TITLES[measure]}
            rows={changes}
            empty={
              comparable
                ? "No previous window figures yet."
                : "There is no previous window for 90 days."
            }
          />
        </div>
      </div>
    </Section>
  );
}
