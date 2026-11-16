import { ItemImage } from "@/components/item-image";
import { Section } from "@/components/page";
import { Hint } from "@/components/ui/hint";
import { counted, num } from "@/lib/format";
import { cn } from "@/lib/utils";
import { capitalize } from "@/lib/world";
import type {
  KuvaWeeklySummary,
  ResetChecklistVendor,
  ResetWindow,
  VendorWeeklyOffer,
} from "@/types";
import { ResetChecklistCountdown } from "./reset-countdown";
import { ResetChecklistTile, ResetChecklistTileGrid } from "./tiles";

function VendorWeeklyOfferLine({ offer }: { offer: VendorWeeklyOffer }) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span className="flex min-w-0 items-center gap-1.5">
        {offer.kind === "kuva" ? (
          <>
            {num(offer.amount)} Kuva
            {offer.certainty === "estimated" && (
              <Hint as="span">estimated</Hint>
            )}
          </>
        ) : (
          <>
            <ItemImage imageName={offer.image_name} size={20} />
            <span className="truncate">
              {capitalize(offer.colour)} Archon Shard
            </span>
          </>
        )}
      </span>
      {offer.kind === "kuva" && offer.locked ? (
        <Hint as="span" className="shrink-0">
          Locked
        </Hint>
      ) : (
        <span className="shrink-0">
          {num(offer.bought)}/{num(offer.limit)}
        </span>
      )}
    </div>
  );
}

function ResetChecklistVendorTile({
  vendor,
  showLocked,
}: {
  vendor: ResetChecklistVendor;
  showLocked: boolean;
}) {
  const offer = vendor.weekly_offer;
  const offerHidden =
    vendor.locked || (offer?.kind === "kuva" && offer.locked && !showLocked);
  const status = vendor.locked
    ? "Locked"
    : `${counted(vendor.offers_bought, "offer")} bought`;

  return (
    <ResetChecklistTile>
      <span
        className={cn("font-medium", vendor.locked && "text-muted-foreground")}
      >
        {vendor.name}
      </span>
      <Hint as="span">{[vendor.place, status].filter(Boolean).join(", ")}</Hint>
      {offer && !offerHidden && <VendorWeeklyOfferLine offer={offer} />}
    </ResetChecklistTile>
  );
}

export function ResetChecklistVendorPurchases({
  vendors,
  kuva,
  showLocked,
  reset,
  now,
}: {
  vendors: ResetChecklistVendor[];
  kuva: KuvaWeeklySummary;
  showLocked: boolean;
  reset: ResetWindow;
  now: number;
}) {
  return (
    <Section
      title="Shops"
      description={`${num(kuva.confirmed + kuva.estimated)}/${num(kuva.available)} Kuva bought (${num(kuva.confirmed)} confirmed, ${num(kuva.estimated)} estimated).`}
      action={<ResetChecklistCountdown window={reset} now={now} />}
    >
      <Hint className="mb-3">
        &quot;Confirmed&quot; means Kuva from vendors that sell nothing else
        that has a weekly limit. &quot;Estimated&quot; means Kuva from all other
        vendors, where we try to match purchases by Kuva increasing between
        inventory syncs. This can only be tracked while Merframe is running.
      </Hint>
      <ResetChecklistTileGrid>
        {vendors
          .filter((vendor) => showLocked || !vendor.locked)
          .map((vendor) => (
            <ResetChecklistVendorTile
              key={`${vendor.name}, ${vendor.place}`}
              vendor={vendor}
              showLocked={showLocked}
            />
          ))}
      </ResetChecklistTileGrid>
    </Section>
  );
}
