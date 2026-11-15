import { ExternalLink, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { toast } from "sonner";
import { useShallow } from "zustand/react/shallow";
import { Amount } from "@/components/currency";
import { ItemImage } from "@/components/item-image";
import { ItemSearch } from "@/components/item-search";
import { OrderList } from "@/components/market-order-list";
import { SelectField } from "@/components/option-select";
import { EmptyNote } from "@/components/page";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field } from "@/components/ui/field";
import { Hint } from "@/components/ui/hint";
import { Input } from "@/components/ui/input";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useLatestAttempt } from "@/hooks/use-latest-attempt";
import { api, openExternal, reportError } from "@/lib/bridge";
import { marketUrl, num } from "@/lib/format";
import { notify } from "@/lib/toast";
import { cn } from "@/lib/utils";
import { capitalize } from "@/lib/world";
import { useAppStore } from "@/stores/app-store";
import { useMarketItemsStore } from "@/stores/market-items-store";
import { useMarketListingsStore } from "@/stores/market-listings-store";
import {
  type ListingStars,
  useMarketPanelStore,
} from "@/stores/market-panel-store";
import type { ItemListings, MarketItem, OrderRow, OrderType } from "@/types";

function NumberField({
  id,
  label,
  value,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <Field id={id} label={label}>
      <Input
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        inputMode="numeric"
      />
    </Field>
  );
}

function defaultSubtype(item: MarketItem, wanted: string | null) {
  const subtypes = item.subtypes ?? [];
  if (wanted && subtypes.includes(wanted)) {
    return wanted;
  }
  return subtypes.includes("revealed") ? "revealed" : (subtypes[0] ?? "");
}

function ownOrder(
  rows: OrderRow[],
  item: MarketItem,
  side: OrderType,
  rank: string,
  subtype: string,
) {
  const wantedRank = item.max_rank == null ? null : Number.parseInt(rank, 10);
  const wantedSubtype = (item.subtypes ?? []).length > 0 ? subtype : null;
  return (
    rows.find(
      (row) =>
        row.item_id === item.id &&
        row.order_type === side &&
        row.rank === wantedRank &&
        row.subtype === wantedSubtype,
    ) ?? null
  );
}

function orderAmounts(platinum: string, quantity: string) {
  const plat = Number.parseInt(platinum, 10);
  const count = Number.parseInt(quantity, 10);

  if (!Number.isFinite(plat) || plat <= 0) {
    toast.error("A platinum price is required");
    return null;
  }
  if (!Number.isFinite(count) || count <= 0) {
    toast.error("A quantity of at least 1 is required");
    return null;
  }

  return { plat, count };
}

export function MarketPanel() {
  const { open, request, takeRequest, hide } = useMarketPanelStore(
    useShallow((state) => ({
      open: state.open,
      request: state.request,
      takeRequest: state.takeRequest,
      hide: state.hide,
    })),
  );
  const items = useMarketItemsStore((state) => state.bySlug);
  const searchable = useMarketItemsStore((state) => state.items);
  const rows = useMarketListingsStore((state) => state.orders.rows);
  const signedIn = useAppStore((state) => state.status?.market_account) != null;
  const navigate = useNavigate();
  const [selected, setSelected] = useState<MarketItem | null>(null);
  const [listings, setListings] = useState<ItemListings | null>(null);
  const [loadingOrders, setLoadingOrders] = useState(false);
  const [platinum, setPlatinum] = useState("");
  const [quantity, setQuantity] = useState("1");
  const [rank, setRank] = useState("0");
  const [subtype, setSubtype] = useState("");
  const [amberStars, setAmberStars] = useState("0");
  const [cyanStars, setCyanStars] = useState("0");
  const [side, setSide] = useState<OrderType>("sell");
  const [posting, setPosting] = useState(false);
  const [updating, setUpdating] = useState(false);
  const [prefilled, setPrefilled] = useState<string | null>(null);
  const { attempt } = useLatestAttempt();

  const isMod = selected?.max_rank != null;
  const subtypes = selected?.subtypes ?? [];
  const hasSubtypes = subtypes.length > 0;
  const hasAmberStars = selected?.max_amber_stars != null;
  const hasCyanStars = selected?.max_cyan_stars != null;
  const existing = selected
    ? ownOrder(rows, selected, side, rank, subtype)
    : null;
  const sideLabel = side === "sell" ? "Sell" : "Buy";
  const busy = posting || updating;

  if (existing && existing.id !== prefilled) {
    setPrefilled(existing.id);
    setPlatinum(String(existing.platinum));
    setQuantity(String(existing.quantity));
  }

  const handlePick = useCallback(
    async (
      item: MarketItem,
      side: OrderType = "sell",
      rank: number | null = null,
      subtype: string | null = null,
      stars: ListingStars | null = null,
    ) => {
      const chosenRank = String(rank ?? 0);
      const chosenSubtype = defaultSubtype(item, subtype);
      setSelected(item);
      setLoadingOrders(true);
      setSide(side);
      setRank(chosenRank);
      setSubtype(chosenSubtype);
      setAmberStars(String(stars?.amber ?? 0));
      setCyanStars(String(stars?.cyan ?? 0));
      setPrefilled(null);
      setPlatinum("");
      setQuantity("1");

      await attempt(() => api.marketItemOrders(item.slug), {
        loaded: (next) => {
          setListings(next);
          const own = ownOrder(
            useMarketListingsStore.getState().orders.rows,
            item,
            side,
            chosenRank,
            chosenSubtype,
          );
          if (own) {
            return;
          }

          const lowest = next.sell[0]?.platinum;
          setPlatinum(lowest === undefined ? "" : String(Math.max(1, lowest)));
        },
        failed: () => setListings(null),
        settled: () => setLoadingOrders(false),
      });
    },
    [attempt],
  );

  useEffect(() => {
    if (!request) {
      return;
    }
    takeRequest();
    const item = items.get(request.slug);
    if (item) {
      handlePick(
        item,
        request.side,
        request.rank,
        request.subtype,
        request.stars,
      );
    } else {
      toast.error("warframe.market does not list this item any more");
    }
  }, [request, items, takeRequest, handlePick]);

  const handlePost = useCallback(async () => {
    if (!selected) {
      return;
    }
    if (!signedIn) {
      navigate("/market");
      return;
    }
    const amounts = orderAmounts(platinum, quantity);
    if (!amounts) {
      return;
    }

    const { plat, count } = amounts;
    const modRank = Number.parseInt(rank, 10);
    const amber = Number.parseInt(amberStars, 10);
    const cyan = Number.parseInt(cyanStars, 10);
    if (hasSubtypes && !subtype) {
      toast.error("A type is required for this item");
      return;
    }
    setPosting(true);
    try {
      await api.marketPostOrder({
        item_id: selected.id,
        order_type: side,
        platinum: plat,
        quantity: count,
        visible: true,
        per_trade: selected.bulk_tradable ? 1 : undefined,
        rank: isMod && Number.isFinite(modRank) ? modRank : undefined,
        subtype: hasSubtypes ? subtype : undefined,
        amber_stars:
          hasAmberStars && Number.isFinite(amber) ? amber : undefined,
        cyan_stars: hasCyanStars && Number.isFinite(cyan) ? cyan : undefined,
      });
      notify.success(`${sideLabel} order posted for ${selected.name}`);
      setSelected(null);
      setListings(null);
      setQuantity("1");
    } catch (error) {
      reportError(error);
    } finally {
      setPosting(false);
    }
  }, [
    selected,
    signedIn,
    navigate,
    platinum,
    quantity,
    rank,
    amberStars,
    cyanStars,
    side,
    sideLabel,
    isMod,
    hasSubtypes,
    subtype,
    hasAmberStars,
    hasCyanStars,
  ]);

  const handleUpdate = useCallback(async () => {
    if (!existing || !selected) {
      return;
    }

    const amounts = orderAmounts(platinum, quantity);
    if (!amounts) {
      return;
    }

    setUpdating(true);
    try {
      await api.marketUpdateOrder(existing.id, {
        platinum: amounts.plat,
        quantity: amounts.count,
      });
      notify.success(`${sideLabel} order updated for ${selected.name}`);
    } catch (error) {
      reportError(error);
    } finally {
      setUpdating(false);
    }
  }, [existing, selected, platinum, quantity, sideLabel]);

  const lowestSell = listings?.sell[0]?.platinum ?? null;

  return (
    <aside
      className={cn(
        "bg-card flex w-96 shrink-0 flex-col border-l",
        !open && "hidden",
      )}
    >
      <div className="flex h-14 shrink-0 items-center justify-between border-b px-4">
        <span className="text-foreground font-bold">warframe.market</span>
        <Button
          variant="ghost"
          size="icon"
          onClick={hide}
          aria-label="Close"
          title="Close"
        >
          <X className="size-4" />
        </Button>
      </div>
      <div className="flex min-h-0 flex-1 flex-col gap-3 p-4">
        <ItemSearch
          items={searchable}
          placeholder="Search warframe.market items"
          onPick={handlePick}
        />
        {selected ? (
          <>
            <div className="flex items-center gap-2">
              <ItemImage
                imageName={selected.image_name}
                size={32}
                alt={selected.name}
              />
              <span className="flex-1 truncate font-semibold">
                {selected.name}
              </span>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => openExternal(marketUrl(selected.slug))}
                aria-label="Open on warframe.market"
                title="Open on warframe.market"
              >
                <ExternalLink className="size-4" />
              </Button>
            </div>
            <Tabs
              value={side}
              onValueChange={(value) => setSide(value as OrderType)}
            >
              <TabsList className="w-full">
                <TabsTrigger value="sell">WTS</TabsTrigger>
                <TabsTrigger value="buy">WTB</TabsTrigger>
              </TabsList>
            </Tabs>
            <Hint as="span">
              {side === "sell" ? "Lowest sellers" : "Highest buyers"} in game.
              Click a trader to copy a whisper, a price to use it.
            </Hint>
            <div className="min-h-0 flex-1 overflow-y-auto">
              <OrderList
                item={selected}
                orders={
                  side === "sell"
                    ? (listings?.sell ?? [])
                    : (listings?.buy ?? [])
                }
                loading={loadingOrders}
                side={side}
                best={side === "buy" ? lowestSell : null}
                onPickPrice={(price) => setPlatinum(String(price))}
              />
            </div>
          </>
        ) : (
          <EmptyNote>
            Pick an item here or from a Sell or Buy button to see its
            warframe.market listings.
          </EmptyNote>
        )}
      </div>
      {selected && (
        <div className="flex shrink-0 flex-col gap-2 border-t p-4">
          {existing && (
            <div className="flex items-center gap-2 text-sm">
              <span className="flex flex-1 items-center gap-1">
                You're {side === "sell" ? "selling" : "buying"} this for
                <Amount currency="platinum" plain>
                  {num(existing.platinum)}
                </Amount>
                , {num(existing.quantity)} listed.
              </span>
              <Badge variant={existing.visible ? "accent" : "muted"}>
                {existing.visible ? "visible" : "hidden"}
              </Badge>
            </div>
          )}
          <div className="grid grid-cols-2 gap-2">
            <NumberField
              id="order-platinum"
              label="Platinum"
              value={platinum}
              onChange={setPlatinum}
            />
            <NumberField
              id="order-quantity"
              label="Quantity"
              value={quantity}
              onChange={setQuantity}
            />
            {isMod && (
              <NumberField
                id="order-rank"
                label={`Rank (max ${selected.max_rank})`}
                value={rank}
                onChange={setRank}
              />
            )}
            {hasAmberStars && (
              <NumberField
                id="order-amber-stars"
                label="Amber stars"
                value={amberStars}
                onChange={setAmberStars}
              />
            )}
            {hasCyanStars && (
              <NumberField
                id="order-cyan-stars"
                label="Cyan stars"
                value={cyanStars}
                onChange={setCyanStars}
              />
            )}
            {hasSubtypes && (
              <SelectField
                id="order-subtype"
                label="Type"
                value={subtype}
                options={subtypes.map((option) => ({
                  value: option,
                  label: capitalize(option),
                }))}
                onChange={setSubtype}
              />
            )}
          </div>
          <div className="grid grid-cols-2 gap-2">
            {existing && (
              <Button onClick={handleUpdate} disabled={busy} pending={updating}>
                Update Order
              </Button>
            )}
            <Button
              variant={existing ? "outline" : "default"}
              onClick={handlePost}
              disabled={busy}
              pending={posting}
              className={cn(!existing && "col-span-2")}
            >
              {existing ? "Post New Order" : `Post ${sideLabel} Order`}
            </Button>
          </div>
        </div>
      )}
    </aside>
  );
}
