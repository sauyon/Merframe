import { useNavigate } from "react-router";
import { Countdown } from "@/components/countdown";
import { GameIcon } from "@/components/game-icon";
import { Surface } from "@/components/page";
import { Hint } from "@/components/ui/hint";
import { useAsyncData } from "@/hooks/use-async-data";
import {
  ARBITRATION_RELOAD_EVENTS,
  currentSlot,
  liveSlots,
  slotMission,
  slotPlace,
  upcomingSlot,
} from "@/lib/arbitrations";
import { api } from "@/lib/bridge";
import { secondsUntil } from "@/lib/format";
import { timeLeft } from "@/lib/world";
import { TierBadge } from "@/pages/arbitrations/schedule";
import { useSettings } from "@/stores/app-store";

export function WorldArbitrationCard({ now }: { now: number }) {
  const shown = useSettings().alerts.arbitrations.show_on_world_page;
  const { data } = useAsyncData(
    "arbitrations",
    api.arbitrations,
    ARBITRATION_RELOAD_EVENTS,
  );
  const navigate = useNavigate();
  if (!shown) {
    return null;
  }

  const schedule = data?.schedule ?? null;
  const current = currentSlot(liveSlots(schedule, now), now);
  const upcoming = upcomingSlot(schedule, now);
  const remaining = current ? secondsUntil(current.ends, now) : 0;
  return (
    <Surface
      className="cursor-pointer px-4 py-3 @sm:col-span-full"
      role="link"
      tabIndex={0}
      title="Open the Arbitration schedule"
      onClick={() => navigate("/arbitrations")}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          navigate("/arbitrations");
        }
      }}
    >
      <div className="flex items-center gap-3">
        <span className="bg-secondary/50 flex size-11 shrink-0 items-center justify-center rounded-full">
          <GameIcon name="arbitration" size={28} alt="" />
        </span>
        <div className="flex min-w-0 flex-1 flex-col leading-tight">
          <Hint as="span">Arbitration</Hint>
          {current ? (
            <>
              <span className="text-primary truncate text-lg font-bold">
                {slotMission(current)}
              </span>
              <Hint as="span" className="truncate">
                {slotPlace(current)}
              </Hint>
            </>
          ) : (
            <span className="truncate text-sm">
              {data?.status === "unavailable"
                ? "Forecast unavailable"
                : "Loading the forecast"}
            </span>
          )}
        </div>
        {current && (
          <>
            <TierBadge tier={current.tier} />
            <Countdown seconds={remaining} className="text-base" />
          </>
        )}
      </div>
      {upcoming && (
        <Hint as="span" className="truncate">
          {upcoming.label}: {slotMission(upcoming.slot)},{" "}
          {slotPlace(upcoming.slot)}
          {upcoming.label === "Next match" &&
            ` in ${timeLeft(secondsUntil(upcoming.slot.starts, now))}`}
        </Hint>
      )}
    </Surface>
  );
}
