import { useEffect, useState } from "react";
import { ItemImage } from "@/components/item-image";
import { api, reportError } from "@/lib/bridge";
import { cn } from "@/lib/utils";
import type { BuffSettings, SeenBuff } from "@/types";

/** How often the list picks up buffs newly seen in game. */
const REFRESH_MS = 5000;

/** A buff's name as the game keys it, `ArcaneGraceBuff`, spaced into words. */
function spaced(name: string) {
  return name.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replace(/_/g, " ");
}

type Filters = Pick<
  BuffSettings,
  "buffs_squad" | "buffs_squad_shown" | "buffs_hidden"
>;

/** Whether the strip shows the buff: not hidden, and if a squadmate's, squad buffs are on or
 * its name starts one of the always-shown ones. */
export function isShown(buff: SeenBuff, filters: Filters) {
  if (filters.buffs_hidden.includes(buff.name)) {
    return false;
  }
  return (
    buff.kind !== "squad" ||
    filters.buffs_squad ||
    filters.buffs_squad_shown.some((start) => buff.name.startsWith(start))
  );
}

/** The settings change one click on the buff makes: hides it when shown, else unhides it and,
 * for a squadmate's buff that stays off, adds it to the always-shown ones. */
export function flipped(buff: SeenBuff, filters: Filters): Partial<Filters> {
  if (isShown(buff, filters)) {
    return { buffs_hidden: [...filters.buffs_hidden, buff.name] };
  }
  const unhidden = {
    ...filters,
    buffs_hidden: filters.buffs_hidden.filter((name) => name !== buff.name),
  };
  if (isShown(buff, unhidden)) {
    return { buffs_hidden: unhidden.buffs_hidden };
  }
  return {
    buffs_hidden: unhidden.buffs_hidden,
    buffs_squad_shown: [...filters.buffs_squad_shown, buff.name],
  };
}

/** The buffs seen beside the ability dots since the app started, each one click from hidden
 * or shown. */
export function BuffFilters({
  filters,
  onChange,
}: {
  filters: Filters;
  onChange: (change: Partial<Filters>) => void;
}) {
  const hidden = filters.buffs_hidden;
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
        Buffs seen beside the ability dots show up here, to hide or show any of
        them.
      </p>
    );
  }
  return (
    <div className="flex flex-col gap-2">
      <p className="text-muted-foreground text-sm">
        Click a buff to hide or show it. Squadmates' buffs stay off unless squad
        buffs are on or you show them here; Wisp's motes show by default.
      </p>
      <div className="flex flex-wrap gap-2">
        {buffs.map((buff) => {
          const off = !isShown(buff, filters);
          return (
            <button
              key={buff.name}
              type="button"
              aria-pressed={!off}
              onClick={() => onChange(flipped(buff, filters))}
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
