import { Countdown } from "@/components/countdown";
import { secondsUntil } from "@/lib/format";
import type { ResetWindow } from "@/types";

export function ResetChecklistCountdown({
  window,
  now,
}: {
  window: ResetWindow;
  now: number;
}) {
  return (
    <span className="text-muted-foreground text-xs">
      Resets in <Countdown seconds={secondsUntil(window.expiry, now)} />
    </span>
  );
}
