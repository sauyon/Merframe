import { Section } from "@/components/page";
import { Hint } from "@/components/ui/hint";
import { Progress } from "@/components/ui/progress";
import { num } from "@/lib/format";
import type { ResetChecklistStandingPool, ResetWindow } from "@/types";
import { ResetChecklistCountdown } from "./reset-countdown";
import { ResetChecklistTile, ResetChecklistTileGrid } from "./tiles";

export function ResetChecklistStandingPools({
  pools,
  reset,
  now,
}: {
  pools: ResetChecklistStandingPool[];
  reset: ResetWindow;
  now: number;
}) {
  return (
    <Section
      title="Standing"
      description="What you can still earn today"
      action={<ResetChecklistCountdown window={reset} now={now} />}
    >
      <ResetChecklistTileGrid>
        {pools.map((pool) => (
          <ResetChecklistTile key={pool.name}>
            <span className="truncate font-medium">{pool.name}</span>
            <Hint as="span">
              {num(pool.remaining)} left of {num(pool.cap)}
            </Hint>
            <Progress value={(pool.remaining / pool.cap) * 100} />
          </ResetChecklistTile>
        ))}
      </ResetChecklistTileGrid>
    </Section>
  );
}
