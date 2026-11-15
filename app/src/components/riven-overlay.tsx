import { Check, ExternalLink } from "lucide-react";
import { useEffect } from "react";
import { PolarityIcon } from "@/components/game-icon";
import { GoodRollAttribution, GoodRollBlock } from "@/components/good-roll";
import { ItemImage, prefetchImages } from "@/components/item-image";
import { RivenListings } from "@/components/riven-listings";
import { RivenRollBar } from "@/components/riven-roll-bar";
import { SplicedMark, SpliceOptions } from "@/components/spliced-mark";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { openExternal } from "@/lib/bridge";
import { percent } from "@/lib/format";
import {
  attributeAmount,
  attributeLabel,
  attributeValue,
  gradeClasses,
  gradeVariant,
  rivenAuctionUrl,
} from "@/lib/rivens";
import { cn } from "@/lib/utils";
import type { AttributeGrade, RivenRow } from "@/types";

function rivenName(riven: RivenRow): string {
  return riven.name === null ? riven.weapon : `${riven.weapon} ${riven.name}`;
}

function Attribute({ attribute }: { attribute: AttributeGrade }) {
  return (
    <div className="grid grid-cols-subgrid col-span-3 gap-y-0.5 text-sm items-center">
      <span
        className={cn(
          "w-14 text-right",
          attribute.desired ? "text-accent" : attribute.curse && "text-warning",
        )}
      >
        {attributeValue(attribute) ?? percent(attribute.percentile, 0)}
      </span>
      <span className="col-span-2 flex min-w-0 items-center gap-1">
        <span className={cn("truncate", attribute.desired && "text-accent")}>
          {attributeLabel(attribute)}
        </span>
        <SplicedMark attribute={attribute} />
        {attribute.desired && (
          <Check
            className="text-accent size-3.5 shrink-0"
            aria-label="Desired"
          />
        )}
      </span>
      <span className="text-muted-foreground text-right text-xs">
        {attribute.max !== null &&
          `/${attributeAmount(attribute.unit, attribute.max)}`}
      </span>
      <RivenRollBar attribute={attribute} />
      <span className={cn("text-center", gradeClasses(attribute.grade).text)}>
        {attribute.grade}
      </span>
    </div>
  );
}

function RivenCard({ riven, compact }: { riven: RivenRow; compact: boolean }) {
  const slug = riven.weapon_slug;

  return (
    <div className="flex w-80 max-w-full flex-col rounded-lg border gap-3 px-2 py-1.5">
      <div className="flex items-center gap-2">
        <ItemImage
          imageName={riven.image_name}
          size={compact ? 28 : 32}
          alt={riven.weapon}
        />
        <div className="flex min-w-0 flex-col">
          <span className="flex items-center gap-1 text-sm font-medium">
            {rivenName(riven)}
            {riven.polarity && (
              <PolarityIcon polarity={riven.polarity} size={16} />
            )}
          </span>
        </div>
        <div className="ml-auto flex items-center gap-2">
          <Badge variant={gradeVariant(riven.grade_letter)}>
            {riven.grade_letter}
          </Badge>
          <span className="text-xs">{percent(riven.grade)}</span>
        </div>
      </div>
      <div className="grid grid-cols-[auto_minmax(0,1fr)_auto] gap-1.5">
        {riven.attributes.map((attribute) => (
          <Attribute key={attribute.tag} attribute={attribute} />
        ))}
      </div>
      <SpliceOptions splices={riven.splices} />
      {riven.good_roll && (
        <GoodRollBlock view={riven.good_roll} className="border-t pt-1.5" />
      )}
      {slug && (
        <RivenListings
          weaponSlug={slug}
          attributes={riven.attributes}
          limit={compact ? 3 : 5}
          className="border-t pt-1.5"
        />
      )}
      {slug && !compact && (
        <Button
          variant="outline"
          size="sm"
          className="self-start"
          onClick={() => openExternal(rivenAuctionUrl(slug))}
        >
          <ExternalLink className="size-3.5" />
          Compare on warframe.market
        </Button>
      )}
    </div>
  );
}

export function RivenRerollOverlay({
  before,
  after,
  attribution = null,
  compact = false,
}: {
  before: RivenRow;
  after: RivenRow | null;
  attribution?: string | null;
  compact?: boolean;
}) {
  useEffect(() => {
    prefetchImages([before.image_name]);
  }, [before.image_name]);

  if (!after) {
    return (
      <div className="flex flex-col gap-1">
        <RivenCard riven={before} compact={compact} />
        <GoodRollAttribution attribution={attribution} />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex flex-wrap gap-2">
        <RivenCard riven={before} compact={compact} />
        <RivenCard riven={after} compact={compact} />
      </div>
      <GoodRollAttribution attribution={attribution} />
    </div>
  );
}

export function RivenOverlay({
  rivens,
  attribution = null,
  compact = false,
}: {
  rivens: RivenRow[];
  attribution?: string | null;
  compact?: boolean;
}) {
  useEffect(() => {
    prefetchImages(rivens.map((riven) => riven.image_name));
  }, [rivens]);

  return (
    <div className={cn("flex flex-col", compact ? "gap-1" : "gap-2")}>
      {rivens.map((riven) => (
        <RivenCard key={riven.item_id} riven={riven} compact={compact} />
      ))}
      <GoodRollAttribution attribution={attribution} />
    </div>
  );
}
