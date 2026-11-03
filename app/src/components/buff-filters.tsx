import { useEffect, useState } from "react";
import { ItemImage } from "@/components/item-image";
import { api, reportError } from "@/lib/bridge";
import { toggled } from "@/lib/filters";
import { cn } from "@/lib/utils";
import type { SeenBuff } from "@/types";

/** How often the list picks up buffs newly seen in game. */
const REFRESH_MS = 5000;

/** A buff's name as the game keys it, `ArcaneGraceBuff`, spaced into words. */
function spaced(name: string) {
  return name.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replace(/_/g, " ");
}

/** The buffs seen beside the ability dots since the app started, each one click from hidden. */
export function BuffFilters({
  hidden,
  onChange,
}: {
  hidden: string[];
  onChange: (hidden: string[]) => void;
}) {
  const [seen, setSeen] = useState<SeenBuff[]>([]);
  useEffect(() => {
    let live = true;
    const load = () =>
      api
        .seenBuffs()
        .then((next) => live && setSeen(next))
        .catch(reportError);
    load();
    const timer = window.setInterval(load, REFRESH_MS);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, []);

  const names = new Set(seen.map((buff) => buff.name));
  const buffs = [
    ...seen,
    ...hidden
      .filter((name) => !names.has(name))
      .map((name): SeenBuff => ({ name, kind: "own", icon: null })),
  ];
  if (buffs.length === 0) {
    return (
      <p className="text-muted-foreground text-sm">
        Buffs seen beside the ability dots show up here, to hide any of them.
      </p>
    );
  }
  return (
    <div className="flex flex-col gap-2">
      <p className="text-muted-foreground text-sm">
        Click a buff to hide or show it.
      </p>
      <div className="flex flex-wrap gap-2">
        {buffs.map((buff) => {
          const off = hidden.includes(buff.name);
          return (
            <button
              key={buff.name}
              type="button"
              aria-pressed={!off}
              onClick={() => onChange(toggled(hidden, buff.name))}
              className={cn(
                "flex items-center gap-1.5 rounded-md border px-2 py-1 text-sm",
                off && "text-muted-foreground line-through opacity-60",
              )}
            >
              {buff.icon && <ItemImage imageName={buff.icon} size={18} />}
              {spaced(buff.name)}
            </button>
          );
        })}
      </div>
    </div>
  );
}
