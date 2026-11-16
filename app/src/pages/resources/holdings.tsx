import { Store } from "lucide-react";
import { memo, useEffect, useMemo, useState } from "react";
import { ItemImage, prefetchImages } from "@/components/item-image";
import { CardGrid, EmptyPanel, ResultCount, Section } from "@/components/page";
import { Pagination } from "@/components/pagination";
import { SearchInput } from "@/components/search-input";
import { usePaged } from "@/hooks/use-paged";
import { counted, num } from "@/lib/format";
import { type HoldingsView, visibleHoldings } from "@/lib/resource-filters";
import { cn } from "@/lib/utils";
import { WikiButton } from "@/pages/foundry/node-details";
import type { CurrencyOffer, CurrencyVendor, HeldResource } from "@/types";

const EMPTY: Record<HoldingsView, string> = {
  held: "The inventory holds no resources.",
  currencies: "The inventory holds no vendor currency.",
};

function HeldCardInner({
  row,
  selected,
  onSelect,
}: {
  row: HeldResource;
  selected: boolean;
  onSelect: (uniqueName: string) => void;
}) {
  const body = (
    <>
      <ItemImage imageName={row.image_name} size={48} alt={row.name} />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="line-clamp-2">{row.name}</span>
        <div className="text-muted-foreground flex justify-between gap-2 text-xs tabular-nums">
          <span>
            <span className="text-foreground">{num(row.owned)}</span> owned
          </span>
          {row.vendors.length > 0 && (
            <span className="flex items-center gap-1">
              <Store className="size-3.5" />
              {counted(row.vendors.length, "vendor", "vendors")}
            </span>
          )}
        </div>
      </div>
    </>
  );
  const card =
    "bg-card flex h-full w-full items-center gap-3 rounded-xl border p-3 text-left transition-colors";
  // Only something a vendor takes has a Spend At panel to open.
  if (row.vendors.length === 0) {
    return <div className={card}>{body}</div>;
  }
  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={() => onSelect(row.unique_name)}
      className={cn(
        card,
        "cursor-pointer",
        selected ? "border-primary bg-secondary/50" : "hover:border-primary/50",
      )}
    >
      {body}
    </button>
  );
}

const HeldCard = memo(HeldCardInner);

// The generator drops repeated offers, so price and costs tell the rest apart.
function offerKey(offer: CurrencyOffer): string {
  const costs = offer.also_costs.map(
    (cost) => `${cost.unique_name}=${cost.count}`,
  );
  return [
    offer.unique_name,
    offer.quantity,
    offer.price,
    offer.credits,
    ...costs,
  ].join(":");
}

function OfferRow({ offer }: { offer: CurrencyOffer }) {
  const extras = [
    ...offer.also_costs.map((cost) => `${num(cost.count)} ${cost.name}`),
    ...(offer.credits ? [`${num(offer.credits)} credits`] : []),
  ];
  return (
    <li className="flex items-center gap-2 px-2 py-1 text-sm">
      <ItemImage
        imageName={offer.image_name}
        size={32}
        alt={offer.name}
        className="bg-transparent"
      />
      <div className="min-w-0 flex-1">
        <div className="truncate">
          {offer.quantity > 1 && `${num(offer.quantity)}× `}
          {offer.name}
        </div>
        {extras.length > 0 && (
          <div className="text-muted-foreground truncate text-xs">
            + {extras.join(", ")}
          </div>
        )}
      </div>
      <span
        className={cn(
          "shrink-0 tabular-nums",
          offer.affordable ? "text-primary" : "text-muted-foreground",
        )}
      >
        {num(offer.price)}
        {offer.affordable && <span className="sr-only"> (affordable)</span>}
      </span>
    </li>
  );
}

function VendorBlock({ vendor }: { vendor: CurrencyVendor }) {
  return (
    <li className="flex flex-col gap-2">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="font-medium">{vendor.vendor}</div>
          {vendor.location && (
            <div className="text-muted-foreground text-sm">
              {vendor.location}
            </div>
          )}
        </div>
        {vendor.wiki_url && <WikiButton url={vendor.wiki_url} />}
      </div>
      {vendor.offers.length === 0 ? (
        <p className="text-muted-foreground text-sm">
          Stock rotates. Check the vendor in game.
        </p>
      ) : (
        <ul className="flex flex-col">
          {vendor.offers.map((offer) => (
            <OfferRow key={offerKey(offer)} offer={offer} />
          ))}
        </ul>
      )}
    </li>
  );
}

function SpendAt({ row }: { row: HeldResource }) {
  return (
    <Section
      title="Spend At"
      description={`${counted(row.vendors.length, "vendor takes", "vendors take")} ${row.name}`}
      className="self-start @3xl:sticky @3xl:top-4"
    >
      <ul className="flex max-h-128 flex-col gap-4 overflow-y-auto">
        {row.vendors.map((vendor) => (
          <VendorBlock key={vendor.id} vendor={vendor} />
        ))}
      </ul>
    </Section>
  );
}

export function Holdings({
  holdings,
  view,
  loading,
}: {
  holdings: HeldResource[];
  view: HoldingsView;
  loading: boolean;
}) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const rows = useMemo(
    () => visibleHoldings(holdings, view, query),
    [holdings, view, query],
  );
  const total = useMemo(
    () => visibleHoldings(holdings, view, "").length,
    [holdings, view],
  );
  const { pageItems, pagination } = usePaged(rows, `${view}:${query}`);
  const spending =
    pageItems.find(
      (row) => row.unique_name === selected && row.vendors.length > 0,
    ) ??
    pageItems.find((row) => row.vendors.length > 0) ??
    null;

  useEffect(() => {
    prefetchImages(pageItems.map((row) => row.image_name));
  }, [pageItems]);

  return (
    <div className="grid gap-4 @3xl:grid-cols-3">
      <Section
        title={view === "held" ? "All Held" : "Vendor Currencies"}
        className={spending ? "@3xl:col-span-2" : "@3xl:col-span-3"}
        action={
          <div className="flex items-center gap-3">
            <ResultCount
              shown={rows.length}
              total={total}
              noun={view === "held" ? "resource" : "currency"}
              plural={view === "held" ? undefined : "currencies"}
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
          {rows.length === 0 ? (
            <EmptyPanel>
              {total === 0 ? EMPTY[view] : "Nothing matches this search."}
            </EmptyPanel>
          ) : (
            <CardGrid
              className={cn(
                "transition-opacity @5xl:grid-cols-2 @7xl:grid-cols-3",
                loading && "opacity-60",
              )}
            >
              {pageItems.map((row) => (
                <HeldCard
                  key={row.unique_name}
                  row={row}
                  selected={row === spending}
                  onSelect={setSelected}
                />
              ))}
            </CardGrid>
          )}
          <Pagination {...pagination} />
        </div>
      </Section>

      {spending && <SpendAt row={spending} />}
    </div>
  );
}
