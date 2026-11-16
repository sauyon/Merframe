import { Archive, CheckCircle2, Dna, Hourglass, Star } from "lucide-react";
import { memo } from "react";
import { FavouriteStar } from "@/components/favourite-star";
import { GameIcon } from "@/components/game-icon";
import { ItemImage } from "@/components/item-image";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { counted, num, percent } from "@/lib/format";
import { occurrenceKeys } from "@/lib/keys";
import { cn } from "@/lib/utils";
import type { FoundryItem } from "@/types";

function ComponentTile({
  component,
}: {
  component: FoundryItem["components"][number];
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          className={cn(
            "bg-background relative flex size-11 items-center justify-center rounded-full border",
            component.enough ? "border-primary border-2" : "opacity-60",
          )}
        >
          <ItemImage
            imageName={component.image_name}
            size={30}
            className="bg-transparent"
          />
          {component.favourite && (
            <Star
              className="text-accent absolute -top-1 -right-1 size-3.5"
              fill="currentColor"
            />
          )}
          {component.owned > 0 && (
            <span className="bg-secondary text-secondary-foreground absolute -right-1 -bottom-1 min-w-5 rounded-full px-1 text-center text-xs leading-4 font-medium tabular-nums">
              {component.owned > 999 ? "999+" : component.owned}
            </span>
          )}
        </span>
      </TooltipTrigger>
      <TooltipContent className="flex flex-col gap-1">
        <span>{component.name}</span>
        <span className="tabular-nums">
          {num(component.owned)} of {num(component.required)}
        </span>
        {component.owned_relics.length > 0 && (
          <ul className="flex flex-col gap-1">
            {component.owned_relics.map((relic) => (
              <li
                key={relic.unique_name}
                className="flex items-center gap-1.5 tabular-nums"
              >
                <ItemImage
                  imageName={relic.image_name}
                  size={18}
                  alt={relic.name}
                />
                <span className="flex-1">{relic.name}</span>
                <span>x{num(relic.owned)}</span>
                <span>{percent(relic.chance / 100, 1)}</span>
              </li>
            ))}
          </ul>
        )}
      </TooltipContent>
    </Tooltip>
  );
}

function FoundryCardInner({
  item,
  onOpen,
}: {
  item: FoundryItem;
  onOpen: (uniqueName: string) => void;
}) {
  const keys = occurrenceKeys(
    item.components.map((component) => component.unique_name),
  );
  return (
    <div className="relative h-full">
      <FavouriteStar
        uniqueName={item.unique_name}
        favourite={item.favourite}
        size="small"
        className="absolute top-2.5 left-2.5"
      />
      <button
        type="button"
        onClick={() => onOpen(item.unique_name)}
        className={cn(
          "bg-card hover:border-primary/50 flex h-full w-full cursor-pointer flex-col gap-3 rounded-xl border p-3 text-left transition-colors",
          item.progress.owned && "bg-secondary/50",
        )}
      >
        <div className="flex items-center justify-center gap-2">
          <span>{item.name}</span>
          {item.mastered && (
            <GameIcon name="mastered" size={16} alt="Mastered" />
          )}
          {item.archon_shards !== null && item.archon_shards > 0 && (
            <span
              title={`${counted(item.archon_shards, "Archon Shard")} installed`}
              className="text-muted-foreground flex items-center gap-1 text-xs tabular-nums"
            >
              <GameIcon name="archon-shard" size={16} alt="Archon Shards" />
              {item.archon_shards}
            </span>
          )}
        </div>
        <div className="flex items-center gap-3">
          <ItemImage
            imageName={item.image_name}
            size={96}
            alt={item.name}
            className="bg-transparent"
          />
          <div className="flex flex-1 flex-wrap items-center justify-center gap-2">
            {item.components.map((component, position) => (
              <ComponentTile key={keys[position]} component={component} />
            ))}
          </div>
        </div>
        <div className="text-muted-foreground flex min-h-5 flex-wrap items-center justify-center gap-x-3 gap-y-1 text-xs">
          {item.prime && item.prime.vault !== "unknown" && (
            <span
              className={cn(
                "flex items-center gap-1",
                item.prime.vault === "vaulted" && "text-vaulted",
              )}
            >
              <Archive className="size-3.5" />
              {item.prime.vault === "vaulted" ? "Vaulted" : "Unvaulted"}
            </span>
          )}
          {item.progress.pending && (
            <span className="text-accent flex items-center gap-1">
              <Hourglass className="size-3.5" />
              Building
            </span>
          )}
          {item.progress.owned && (
            <span className="text-primary flex items-center gap-1">
              <CheckCircle2 className="size-3.5" />
              Owned
            </span>
          )}
          {!item.progress.owned && item.progress.ready_to_build && (
            <span className="text-accent">Ready to build</span>
          )}
          {item.mastery.required !== null && item.mastery.required > 1 && (
            <span>MR {item.mastery.required}</span>
          )}
          {item.helminth?.subsumed && (
            <span className="flex items-center gap-1">
              <Dna className="size-3.5" />
              {item.helminth.ability} subsumed
            </span>
          )}
          {item.crafts_into !== null && item.crafts_into.length > 0 && (
            <span title={`Used to craft ${item.crafts_into.join(", ")}`}>
              Crafting stock
            </span>
          )}
        </div>
      </button>
    </div>
  );
}

export const FoundryCard = memo(FoundryCardInner);
