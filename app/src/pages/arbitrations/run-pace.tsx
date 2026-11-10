import { Figure } from "@/components/figure";
import { Hint } from "@/components/ui/hint";
import { Progress } from "@/components/ui/progress";
import {
  type ArbitrationRunCycle,
  type ArbitrationRunCyclePace,
  cyclePace,
  cycleRows,
  elapsedText,
} from "@/lib/arbitration-runs";
import { num } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { ArbitrationRun } from "@/types";

const PACE_FIGURES: [keyof ArbitrationRunCyclePace, string][] = [
  ["fastest", "Fastest cycle"],
  ["median", "Median cycle"],
  ["slowest", "Slowest cycle"],
];

const COLUMN = {
  index: "w-6",
  reached: "w-16",
  length: "w-16 text-right",
  drones: "w-12 text-right",
};

function CycleRow({
  cycle,
  share,
  counted,
}: {
  cycle: ArbitrationRunCycle;
  share: number | null;
  counted: boolean;
}) {
  return (
    <li className="flex items-center gap-3 text-sm">
      <span className={cn(COLUMN.index, "text-muted-foreground font-mono")}>
        {cycle.index}
      </span>
      <span className={cn(COLUMN.reached, "font-mono")}>
        {elapsedText(cycle.at_secs)}
      </span>
      <span className="flex flex-1 items-center gap-2">
        {share === null ? (
          <span className="flex-1" />
        ) : (
          <Progress value={share * 100} aria-hidden="true" />
        )}
        <span className={cn(COLUMN.length, "font-mono")}>
          {cycle.duration_secs === null
            ? "-"
            : elapsedText(cycle.duration_secs)}
        </span>
      </span>
      {counted && (
        <span className={cn(COLUMN.drones, "font-mono")}>
          {num(cycle.drones)}
        </span>
      )}
    </li>
  );
}

export function ArbitrationRunPace({ run }: { run: ArbitrationRun }) {
  const cycles = cycleRows(run);
  if (cycles.length === 0) {
    return null;
  }

  const pace = cyclePace(run);
  const counted = cycles.some((cycle) => cycle.drones !== null);
  return (
    <div className="flex flex-col gap-3">
      <div className="grid grid-cols-3 gap-x-6 gap-y-3">
        {PACE_FIGURES.map(([key, label]) => (
          <Figure
            key={key}
            label={label}
            value={pace ? elapsedText(pace[key]) : "-"}
          />
        ))}
      </div>
      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-3">
          <Hint as="span" className={COLUMN.index}>
            Cycle
          </Hint>
          <Hint as="span" className={COLUMN.reached}>
            Reached
          </Hint>
          <Hint as="span" className="flex-1 text-right">
            Length
          </Hint>
          {counted && (
            <Hint as="span" className={COLUMN.drones}>
              Drones
            </Hint>
          )}
        </div>
        <ul className="flex max-h-64 flex-col gap-1.5 overflow-y-auto">
          {cycles.map((cycle) => (
            <CycleRow
              key={cycle.index}
              cycle={cycle}
              share={
                pace && cycle.duration_secs !== null
                  ? cycle.duration_secs / pace.slowest
                  : null
              }
              counted={counted}
            />
          ))}
        </ul>
      </div>
    </div>
  );
}
