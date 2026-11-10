import { Figure } from "@/components/figure";
import { elapsedText, perMinute, rateText } from "@/lib/arbitration-runs";
import { num } from "@/lib/format";
import type { ArbitrationRun } from "@/types";

export function ArbitrationRunFigures({
  run,
  elapsedSecs,
}: {
  run: ArbitrationRun;
  elapsedSecs: number;
}) {
  const rate = perMinute(run.drone_spawns, elapsedSecs);
  return (
    <>
      <Figure label="Elapsed" value={elapsedText(elapsedSecs)} />
      <Figure label="Reward cycles" value={num(run.reward_cycles)} />
      <Figure label="Drones" value={num(run.drone_spawns)} />
      <Figure label="Drones per minute" value={rateText(rate, 2)} />
    </>
  );
}
