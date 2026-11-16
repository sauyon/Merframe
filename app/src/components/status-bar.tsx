import { Check, Globe, X } from "lucide-react";
import { useShallow } from "zustand/react/shallow";
import { ModeToggle } from "@/components/ui/theme-toggle";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useNow } from "@/hooks/use-now";
import { ago, countdown, counted, secondsUntil } from "@/lib/format";
import { nodeLabel } from "@/lib/world";
import { useAppStore } from "@/stores/app-store";

const FISSURE_PREVIEW = 8;

export function StatusBar() {
  const { status, world, ready } = useAppStore(
    useShallow((state) => ({
      status: state.status,
      world: state.world,
      ready: state.ready,
    })),
  );
  const now = useNow();

  const detected = status?.game_detected ?? false;
  const fissures = world?.fissures ?? [];
  const nextTimer = world?.timers[0];
  const synced = status?.last_scan_at ?? null;
  const source = status?.source ?? "none";

  const inventoryLabel = () => {
    if (!ready) {
      return "Starting...";
    }
    if (source === "cached") {
      const from = `showing inventory from ${ago(status?.last_sync_at ?? null)}`;
      return detected
        ? `Waiting for a sync, ${from}`
        : `Game not running, ${from}`;
    }
    if (source === "live") {
      return `Inventory synced ${ago(synced)}`;
    }
    return detected
      ? "Waiting for an inventory sync"
      : "Inventory not read yet";
  };

  return (
    <header className="bg-background flex h-14 shrink-0 items-center gap-4 border-b px-4">
      <div className="flex items-center gap-2">
        {detected ? (
          <Check className="size-4 text-primary" />
        ) : (
          <X className="size-4 text-muted" />
        )}
        <span className="text-sm">
          {detected ? "Warframe running" : "Warframe not running"}
        </span>
      </div>

      <span className="text-muted-foreground hidden text-sm md:inline">
        {inventoryLabel()}
      </span>

      {status?.last_scan_error && (
        <span className="text-destructive max-w-96 truncate text-sm">
          {status.last_scan_error}
        </span>
      )}

      <div className="ml-auto flex items-center gap-3">
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="text-muted-foreground hidden items-center gap-2 text-sm lg:flex">
              <Globe className="size-4" />
              {counted(fissures.length, "fissure")}
              {nextTimer && (
                <>
                  <span className="text-muted-foreground/50">|</span>
                  {nextTimer.name} {nextTimer.state}
                  <span className="text-accent">
                    {countdown(secondsUntil(nextTimer.ends, now))}
                  </span>
                </>
              )}
            </span>
          </TooltipTrigger>
          <TooltipContent className="max-w-none">
            {fissures.length === 0 ? (
              "No active fissures"
            ) : (
              <ul className="flex flex-col gap-0.5">
                {fissures.slice(0, FISSURE_PREVIEW).map((fissure) => (
                  <li key={`${fissure.node_id}-${fissure.expiry}`}>
                    {fissure.tier} {fissure.mission_name} at{" "}
                    {nodeLabel(fissure.node_name, fissure.node_id)}
                    {fissure.steel_path ? " (Steel Path)" : ""}
                    {fissure.is_storm ? " (Void Storm)" : ""},{" "}
                    {countdown(secondsUntil(fissure.expiry, now))} left
                  </li>
                ))}
                {fissures.length > FISSURE_PREVIEW && (
                  <li className="opacity-70">
                    and {fissures.length - FISSURE_PREVIEW} more
                  </li>
                )}
              </ul>
            )}
          </TooltipContent>
        </Tooltip>
        <ModeToggle />
      </div>
    </header>
  );
}
