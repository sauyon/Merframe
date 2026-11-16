import { useEffect, useRef } from "react";
import { Amount } from "@/components/currency";
import { ItemImage } from "@/components/item-image";
import { Progress } from "@/components/ui/progress";
import { counted, num } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { DucateringEntry, DucateringProgress } from "@/types";

const COPY: Record<
  DucateringProgress["state"]["state"],
  { title: string; description: string }
> = {
  waiting: {
    title: "Ducatering",
    description: "Waiting for the Ducat Kiosk to show its parts.",
  },
  searching: {
    title: "Ducatering",
    description: "Names are copied to your clipboard.",
  },
  finished: {
    title: "Ducatering finished",
    description: "Every item is in the sell list. Press Sell in the kiosk.",
  },
};

const EMPTY_COPY = {
  title: "Ducatering",
  description: "No parts match the Ducatering filters.",
};

function copy(
  count: number,
  state: DucateringProgress["state"]["state"],
  clipboard: boolean,
) {
  if (count === 0) {
    return EMPTY_COPY;
  }
  if (state === "searching" && !clipboard) {
    return { ...COPY.searching, description: null };
  }
  return COPY[state];
}

function Prices({ entry, size }: { entry: DucateringEntry; size: number }) {
  return (
    <span className="flex shrink-0 flex-col items-end text-xs">
      <Amount currency="platinum" size={size}>
        {num(entry.plat)}
      </Amount>
      <Amount currency="ducats" size={size}>
        {num(entry.ducats)}
      </Amount>
    </span>
  );
}

export function DucateringOverlay({
  progress,
  clipboard,
  compact = false,
}: {
  progress: DucateringProgress;
  clipboard: boolean;
  compact?: boolean;
}) {
  const { state, items, sold, ducats } = progress;
  const done = state.state === "finished" ? items.length : state.position;
  const current = state.state === "searching" ? items[state.position] : null;
  const soldCount = sold.reduce((sum, sale) => sum + sale.count, 0);
  const { title, description } = copy(items.length, state.state, clipboard);
  const list = useRef<HTMLUListElement>(null);
  useEffect(() => {
    const entry = list.current?.children[done];
    if (list.current && entry instanceof HTMLElement) {
      list.current.scrollTop = entry.offsetTop - list.current.offsetTop;
    }
  }, [done]);
  return (
    <div className="flex flex-col gap-3">
      {compact && (
        <div className="flex flex-col">
          <span className="font-semibold">{title}</span>
          {description && (
            <span className="text-muted-foreground text-xs">{description}</span>
          )}
        </div>
      )}
      {current && (
        <div className="bg-muted/40 flex items-center gap-4 rounded-xl border p-4">
          <ItemImage
            imageName={current.item.image_name}
            size={compact ? 48 : 64}
          />
          <span className="flex min-w-0 flex-1 flex-col gap-1">
            <span
              className={cn(
                "truncate font-semibold",
                compact ? "text-base" : "text-xl",
              )}
            >
              {num(current.count)}x {current.item.name}
            </span>
            <span className="text-muted-foreground flex items-center gap-1 text-sm">
              For
              <Amount currency="ducats" plain>
                {num(current.count * current.ducats)}
              </Amount>
            </span>
          </span>
          <Prices entry={current} size={14} />
        </div>
      )}
      {items.length > 0 && <Progress value={(done / items.length) * 100} />}
      <ul
        ref={list}
        className={cn(
          "-mr-2 flex scroll-smooth flex-col gap-1 overflow-y-auto pr-2",
          compact ? "max-h-48" : "max-h-72",
        )}
      >
        {items.map((entry, index) => (
          <li
            key={entry.item.unique_name}
            className={cn(
              "flex items-center gap-3 rounded-md border p-2",
              current !== null && index === done && "border-primary",
              index < done && "opacity-50",
            )}
          >
            <ItemImage imageName={entry.item.image_name} size={28} />
            <span className="min-w-0 flex-1 truncate text-sm">
              {num(entry.count)}x {entry.item.name}
            </span>
            <Prices entry={entry} size={12} />
          </li>
        ))}
      </ul>
      {soldCount > 0 && (
        <div className="text-muted-foreground flex items-center justify-end gap-1 text-xs">
          <span>Sold {counted(soldCount, "item")}</span> for
          <Amount currency="ducats" size={14}>
            {num(ducats)}
          </Amount>
        </div>
      )}
    </div>
  );
}
