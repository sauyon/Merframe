import type { ColumnDef } from "@tanstack/react-table";
import { TriangleAlert } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { DataTable } from "@/components/data-table";
import { FilterGrid } from "@/components/filter-grid";
import { FilterSelect } from "@/components/filter-select";
import { ItemImage } from "@/components/item-image";
import { Badge } from "@/components/ui/badge";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { Dash } from "@/components/ui/dash";
import {
  DropdownMenuItem,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";
import { useFilters } from "@/hooks/use-filters";
import { itemAt } from "@/lib/arrays";
import { api } from "@/lib/bridge";
import type { FilterOption, YesNo } from "@/lib/filters";
import { dateTime, num } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useSettings } from "@/stores/app-store";
import type {
  ItemSummary,
  MarketCategory,
  MarketOrders,
  OrderRow,
  OrderType,
  Pricing,
} from "@/types";
import { ListingToolbar, RowActions, VisibleBadge } from "./listing-controls";

const CATEGORIES: { value: MarketCategory; label: string }[] = [
  { value: "parts", label: "Parts" },
  { value: "relics", label: "Relics" },
  { value: "mods", label: "Mods" },
  { value: "arcanes", label: "Arcanes" },
  { value: "sets", label: "Sets" },
  { value: "misc", label: "Misc" },
];

const SIDES: { value: OrderType; label: string }[] = [
  { value: "sell", label: "Sell" },
  { value: "buy", label: "Buy" },
];

const MISSING: FilterOption<YesNo>[] = [
  { value: "yes", label: "Missing items" },
  { value: "no", label: "Fully stocked" },
];

const PRICING_MARKS: Record<Pricing, { label: string; tone: string }> = {
  overpriced: { label: "Overpriced", tone: "text-warning" },
  underpriced: { label: "Underpriced", tone: "text-vaulted" },
};

const PRICING: FilterOption<Pricing>[] = (
  Object.keys(PRICING_MARKS) as Pricing[]
).map((value) => ({ value, label: PRICING_MARKS[value].label }));

interface OrderFilters {
  category: MarketCategory | null;
  side: OrderType | null;
  missing: YesNo | null;
  pricing: Pricing | null;
}

interface ListedOrder {
  order: OrderRow;
  item: ItemSummary;
}

const PRICING_COLUMN: ColumnDef<ListedOrder> = {
  id: "pricing",
  accessorFn: ({ order }) => order.pricing,
  header: "Pricing",
  cell: ({ row }) => {
    const { pricing } = row.original.order;
    if (pricing === null) {
      return <Dash />;
    }

    const mark = PRICING_MARKS[pricing];
    return <span className={cn("font-medium", mark.tone)}>{mark.label}</span>;
  },
};

function detail(order: OrderRow) {
  if (order.rank !== null) {
    return `rank ${order.rank}`;
  }
  return order.subtype;
}

export function OrdersTable({
  orders,
  onCompare,
  onRefresh,
  onSetVisibility,
  runMarketAction,
}: {
  orders: MarketOrders;
  onCompare: (slug: string, side: OrderType, rank: number | null) => void;
  onRefresh: () => void;
  onSetVisibility: (visible: boolean) => void;
  runMarketAction: (promise: Promise<unknown>, message: string) => void;
}) {
  const { items, rows } = orders;
  const priceCheck = useSettings().market_price_check;
  const [confirming, setConfirming] = useState<"fix" | "remove" | null>(null);
  const filters = useFilters<OrderFilters>({
    category: null,
    side: null,
    missing: null,
    pricing: null,
  });
  const { category, side, missing, pricing } = filters.values;

  const entries = useMemo(
    () => rows.map((order) => ({ order, item: itemAt(items, order.item) })),
    [items, rows],
  );

  const filteredEntries = useMemo(
    () =>
      entries.filter(({ order }) => {
        if (category && order.category !== category) {
          return false;
        }
        if (side && order.order_type !== side) {
          return false;
        }
        if (missing === "yes" && !order.show_warning) {
          return false;
        }
        if (missing === "no" && order.show_warning) {
          return false;
        }
        if (pricing && order.pricing !== pricing) {
          return false;
        }
        return true;
      }),
    [entries, category, side, missing, pricing],
  );

  const columns = useMemo<ColumnDef<ListedOrder>[]>(
    () => [
      {
        id: "order_type",
        accessorFn: ({ order }) => order.order_type,
        header: "Side",
        cell: ({ row }) => {
          const side = row.original.order.order_type;
          return (
            <Badge variant={side === "sell" ? "default" : "secondary"}>
              {side}
            </Badge>
          );
        },
      },
      {
        id: "name",
        accessorFn: ({ item }) => item.name,
        header: "Item",
        enableHiding: false,
        meta: { wrap: true },
        cell: ({ row }) => {
          const { order, item } = row.original;
          return (
            <>
              <ItemImage
                imageName={item.image_name}
                size={24}
                alt={item.name}
              />
              <span className="font-medium">{item.name}</span>
              {detail(order) && <Badge variant="muted">{detail(order)}</Badge>}
            </>
          );
        },
      },
      {
        id: "category",
        accessorFn: ({ order }) => order.category,
        header: "Group",
        cell: ({ row }) => (
          <span className="text-muted-foreground">
            {row.original.order.category}
          </span>
        ),
      },
      {
        id: "platinum",
        accessorFn: ({ order }) => order.platinum,
        header: "Plat",
        meta: { numeric: true },
        cell: ({ row }) => (
          <span className="text-primary font-bold">
            {num(row.original.order.platinum)}
          </span>
        ),
      },
      {
        id: "lowest",
        accessorFn: ({ order }) => order.lowest?.plat ?? null,
        header: "Lowest",
        meta: { numeric: true, label: "Lowest price" },
        cell: ({ row }) => {
          const { lowest } = row.original.order;
          return lowest === null ? (
            <Dash />
          ) : (
            <span>
              {num(lowest.plat)}
              {lowest.from === "rank_zero" ? "+" : ""}
            </span>
          );
        },
      },
      ...(priceCheck ? [PRICING_COLUMN] : []),
      {
        id: "quantity",
        accessorFn: ({ order }) => order.quantity,
        header: "Qty",
        meta: { numeric: true, label: "Quantity" },
        cell: ({ row }) => num(row.original.order.quantity),
      },
      {
        id: "owned",
        accessorFn: ({ order }) => order.owned,
        header: "Owned",
        meta: { numeric: true },
        cell: ({ row }) => {
          const { owned, show_warning } = row.original.order;
          return show_warning ? (
            <Badge variant="warning">
              <TriangleAlert />
              {num(owned)}
            </Badge>
          ) : (
            num(owned)
          );
        },
      },
      {
        id: "visible",
        accessorFn: ({ order }) => order.visible,
        header: "Visible",
        cell: ({ row }) => (
          <VisibleBadge visible={row.original.order.visible} />
        ),
      },
      {
        id: "updated_at",
        accessorFn: ({ order }) => order.updated_at,
        header: "Updated",
        cell: ({ row }) => dateTime(row.original.order.updated_at),
      },
      {
        id: "actions",
        header: "",
        enableSorting: false,
        enableHiding: false,
        cell: ({ row }) => {
          const { order, item } = row.original;
          const slug = item.market_slug;
          const priceToMatch =
            priceCheck && order.pricing !== null ? order.lowest : null;
          return (
            <RowActions label="Order actions">
              {slug && (
                <DropdownMenuItem
                  onSelect={() => onCompare(slug, order.order_type, order.rank)}
                >
                  Compare on the market
                </DropdownMenuItem>
              )}
              {order.show_warning && (
                <DropdownMenuItem
                  onSelect={() =>
                    runMarketAction(
                      api.marketFixOrders(order.id),
                      "Listing matched to the inventory",
                    )
                  }
                >
                  Fix the missing items
                </DropdownMenuItem>
              )}
              <DropdownMenuSeparator />
              {priceToMatch && (
                <DropdownMenuItem
                  onSelect={() =>
                    runMarketAction(
                      api.marketUpdateOrder(order.id, {
                        platinum: priceToMatch.plat,
                      }),
                      "Price set to the lowest",
                    )
                  }
                >
                  Set price to the lowest ({num(priceToMatch.plat)})
                </DropdownMenuItem>
              )}
              <DropdownMenuItem
                onSelect={() =>
                  runMarketAction(
                    api.marketUpdateOrder(order.id, {
                      platinum: Math.max(1, order.platinum - 1),
                    }),
                    "Price lowered by 1",
                  )
                }
              >
                Lower price by 1
              </DropdownMenuItem>
              <DropdownMenuItem
                onSelect={() =>
                  runMarketAction(
                    api.marketUpdateOrder(order.id, {
                      visible: !order.visible,
                    }),
                    order.visible ? "Order hidden" : "Order shown",
                  )
                }
              >
                {order.visible ? "Hide order" : "Show order"}
              </DropdownMenuItem>
              <DropdownMenuItem
                onSelect={() =>
                  runMarketAction(
                    api.marketCloseOrder(order.id, 1),
                    "Order closed for one unit",
                  )
                }
              >
                Mark one as sold
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                variant="destructive"
                onSelect={() =>
                  runMarketAction(
                    api.marketDeleteOrder(order.id),
                    "Order deleted",
                  )
                }
              >
                Delete order
              </DropdownMenuItem>
            </RowActions>
          );
        },
      },
    ],
    [onCompare, priceCheck, runMarketAction],
  );

  const missingCount = rows.filter((row) => row.show_warning).length;

  const handleBulk = useCallback(() => {
    if (confirming === "fix") {
      runMarketAction(
        api.marketFixOrders(),
        "Listings matched to the inventory",
      );
    }
    if (confirming === "remove") {
      runMarketAction(api.marketRemoveAll(), "All orders deleted");
    }
    setConfirming(null);
  }, [confirming, runMarketAction]);

  return (
    <>
      <ConfirmDialog
        open={confirming !== null}
        onOpenChange={(open) => !open && setConfirming(null)}
        title={
          confirming === "remove"
            ? "Delete Every Order"
            : "Fix the Missing Items"
        }
        description={
          confirming === "remove"
            ? `All ${num(rows.length)} orders on your warframe.market profile are deleted, buy orders included. This cannot be undone.`
            : `${num(missingCount)} sell orders offer more than the account holds. Each one drops to the number owned, and an order for an item you no longer own is deleted.`
        }
        confirmLabel={
          confirming === "remove" ? "Delete All Orders" : "Fix Missing Items"
        }
        destructive={confirming === "remove"}
        onConfirm={handleBulk}
      />
      <DataTable
        tableId="marketOrders"
        columns={columns}
        data={filteredEntries}
        resetKey={JSON.stringify([category, side, missing, pricing])}
        searchPlaceholder="Filter orders"
        searchValue={({ item }) => item.name}
        initialSorting={[{ id: "updated_at", desc: true }]}
        rowKey={({ order }) => order.id}
        emptyMessage="No order matches these filters."
        toolbar={
          <ListingToolbar
            noun="orders"
            empty={rows.length === 0}
            onSetVisibility={onSetVisibility}
            onRefresh={onRefresh}
          >
            <DropdownMenuItem
              disabled={missingCount === 0}
              onSelect={() => setConfirming("fix")}
            >
              Fix all missing items ({num(missingCount)})
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              disabled={rows.length === 0}
              variant="destructive"
              onSelect={() => setConfirming("remove")}
            >
              Remove all orders
            </DropdownMenuItem>
          </ListingToolbar>
        }
        filters={
          <FilterGrid activeFilters={filters.active} onClear={filters.clear}>
            <FilterSelect
              label="Group"
              value={category}
              options={CATEGORIES}
              onChange={(value) => filters.set("category", value)}
            />
            <FilterSelect
              label="Side"
              value={side}
              options={SIDES}
              onChange={(value) => filters.set("side", value)}
            />
            <FilterSelect
              label="Stock"
              value={missing}
              options={MISSING}
              onChange={(value) => filters.set("missing", value)}
            />
            {priceCheck && (
              <FilterSelect
                label="Pricing"
                value={pricing}
                options={PRICING}
                onChange={(value) => filters.set("pricing", value)}
              />
            )}
          </FilterGrid>
        }
      />
    </>
  );
}
