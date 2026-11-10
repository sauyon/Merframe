import { ArbitrationRunFigures } from "@/components/arbitration-run-figures";
import { Figure } from "@/components/figure";
import { GameIcon } from "@/components/game-icon";
import { Hint } from "@/components/ui/hint";
import { useNow } from "@/hooks/use-now";
import {
  liveElapsedSecs,
  runRole,
  runTitle,
  STATUS_LABEL,
} from "@/lib/arbitration-runs";
import { num } from "@/lib/format";
import type { ArbitrationRun } from "@/types";

export function ArbitrationLiveOverlay({ run }: { run: ArbitrationRun }) {
  const now = useNow();
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <GameIcon name="arbitration" size={20} alt="" />
        <span className="text-primary truncate text-base font-bold">
          {runTitle(run)}
        </span>
        <Hint as="span" className="ml-auto shrink-0">
          {STATUS_LABEL[run.status]}, {runRole(run).toLowerCase()}
        </Hint>
      </div>
      <div className="flex flex-wrap gap-x-6 gap-y-2">
        <ArbitrationRunFigures
          run={run}
          elapsedSecs={liveElapsedSecs(run, now)}
        />
        <Figure
          label="Enemies alive"
          value={num(run.samples.at(-1)?.live_agents ?? null)}
        />
      </div>
    </div>
  );
}
