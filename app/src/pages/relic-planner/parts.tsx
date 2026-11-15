import { X } from "lucide-react";
import { type Dispatch, type SetStateAction, useState } from "react";
import { ItemImage, prefetchImages } from "@/components/item-image";
import { Section } from "@/components/page";
import { SearchInput } from "@/components/search-input";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useLoaded } from "@/hooks/use-loaded";
import { api, reportError } from "@/lib/bridge";
import { counted, percent } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { MissingPart } from "@/types";

function holds(parts: MissingPart[], part: MissingPart): boolean {
  return parts.some((entry) => entry.unique_name === part.unique_name);
}

export function WantedParts({
  missing,
  wanted,
  setWanted,
  selected,
  onSelect,
}: {
  missing: MissingPart[];
  wanted: MissingPart[];
  setWanted: Dispatch<SetStateAction<MissingPart[]>>;
  selected: MissingPart | null;
  onSelect: (part: MissingPart) => void;
}) {
  const [query, setQuery] = useState("");
  const search = query.trim().toLowerCase();
  const candidates = missing
    .filter((part) => part.name.toLowerCase().includes(search))
    .slice(0, 40);
  const masteryParts = missing.filter((part) => part.mastery);

  return (
    <Section
      title="Wanted Parts"
      description={`${counted(missing.length, "Prime part")} missing`}
    >
      <div className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center gap-2">
          <SearchInput
            value={query}
            onValueChange={setQuery}
            placeholder="Search missing parts"
            className="min-w-48 flex-1"
          />
          <Button
            variant="outline"
            disabled={masteryParts.every((part) => holds(wanted, part))}
            onClick={() =>
              setWanted((current) => [
                ...current,
                ...masteryParts.filter((part) => !holds(current, part)),
              ])
            }
          >
            Select Missing for Mastery
          </Button>
        </div>
        {wanted.length > 0 && (
          <div className="flex flex-wrap gap-1.5">
            {wanted.map((part) => (
              <Badge key={part.unique_name} variant="accent" asChild>
                <button
                  type="button"
                  className="cursor-pointer"
                  onClick={() =>
                    setWanted((current) =>
                      current.filter(
                        (entry) => entry.unique_name !== part.unique_name,
                      ),
                    )
                  }
                >
                  {part.name}
                  <X className="size-3" />
                </button>
              </Badge>
            ))}
          </div>
        )}
        <ul className="flex max-h-72 flex-col gap-1 overflow-y-auto">
          {candidates.map((part) => (
            <li key={part.unique_name}>
              <button
                type="button"
                onClick={() => {
                  onSelect(part);
                  setWanted((current) =>
                    holds(current, part) ? current : [...current, part],
                  );
                }}
                className={cn(
                  "hover:bg-secondary/60 w-full cursor-pointer truncate rounded-md px-2 py-1 text-left text-sm",
                  selected?.unique_name === part.unique_name && "bg-secondary",
                )}
              >
                {part.name}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </Section>
  );
}

export function PartSources({ part }: { part: MissingPart | null }) {
  const sources =
    useLoaded(
      part?.unique_name ?? null,
      async (path) => {
        const relics = await api.relicsFor(path);
        prefetchImages(relics.map((source) => source.image_name));
        return relics;
      },
      (cause) => {
        reportError(cause);
        return [];
      },
    ) ?? [];

  return (
    <Section
      title="Drops From"
      description={part ? part.name : "No part selected."}
    >
      <ul className="flex flex-col gap-1.5">
        {sources.length === 0 && (
          <li className="text-muted-foreground text-sm">
            No relic sources loaded.
          </li>
        )}
        {sources.map((source) => (
          <li
            key={`${source.relic}-${source.rarity}`}
            className="flex items-center gap-2 rounded-md border px-2 py-1.5 text-sm"
          >
            <ItemImage
              imageName={source.image_name}
              size={24}
              alt={source.relic}
            />
            <span className="flex-1 truncate">{source.relic}</span>
            <Badge variant="muted">{source.rarity}</Badge>
            <span className="text-muted-foreground w-14 text-right text-xs">
              {percent(source.chance / 100)}
            </span>
            <span className="text-accent w-10 text-right text-xs">
              x{source.owned}
            </span>
          </li>
        ))}
      </ul>
    </Section>
  );
}
