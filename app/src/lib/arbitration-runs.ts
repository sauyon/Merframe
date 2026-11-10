import { slotMission, slotPlace } from "@/lib/arbitrations";
import { median } from "@/lib/arrays";
import type { FilterOption } from "@/lib/filters";
import { countdown, dateTime } from "@/lib/format";
import type {
  ArbitrationRun,
  ArbitrationRunResourceBooster,
  ArbitrationRunResourceBoosterKind,
  ArbitrationRunSample,
  ArbitrationRunStatus,
} from "@/types";

const RATE_WINDOW_SECS = 60;
const RATE_MIN_SPAN_SECS = 30;

export const STATUS_LABEL: Record<ArbitrationRunStatus, string> = {
  recording: "Recording",
  completed: "Completed",
  failed: "Failed",
  aborted: "Aborted",
  ended: "Ended",
  interrupted: "Interrupted",
};

const BOOSTER_LABEL: Record<ArbitrationRunResourceBoosterKind, string> = {
  amount: "Resource Booster",
  drop_chance: "Resource Drop Chance Booster",
  drop_chance_blessing: "Resource Drop Chance Blessing",
};

export function boosterText(
  booster: ArbitrationRunResourceBooster,
  elapsedSecs: number,
): string {
  const label = BOOSTER_LABEL[booster.kind];
  return booster.expires_secs < elapsedSecs
    ? `${label}, ran out at ${elapsedText(booster.expires_secs)}`
    : label;
}

export function runTitle(run: ArbitrationRun): string {
  return `${slotMission(run)}, ${slotPlace(run)}`;
}

export function runDate(run: ArbitrationRun): string {
  return run.started_at ? dateTime(run.started_at) : "Date unknown";
}

export function runRole(run: ArbitrationRun): string {
  if (run.host === null) {
    return "Role unknown";
  }

  return run.host ? "Host" : "Client";
}

export interface ArbitrationEarningsSummary {
  runs: number;
  vitus: number;
  perHour: number | null;
}

export function earningsSummary(
  runs: ArbitrationRun[],
): ArbitrationEarningsSummary {
  const known = runs.flatMap((run) =>
    run.vitus === null ? [] : [{ vitus: run.vitus, secs: run.duration_secs }],
  );
  const timed = known.filter((run) => run.secs > 0);
  const total = (list: { vitus: number }[]) =>
    list.reduce((sum, run) => sum + run.vitus, 0);
  const secs = timed.reduce((sum, run) => sum + run.secs, 0);
  return {
    runs: known.length,
    vitus: total(known),
    perHour: secs > 0 ? (total(timed) * 3600) / secs : null,
  };
}

export function elapsedText(secs: number): string {
  const whole = Math.floor(secs);
  return whole <= 0 ? "0s" : countdown(whole);
}

export function clockText(secs: number): string {
  const whole = Math.max(0, Math.floor(secs));
  const minutes = Math.floor(whole / 60);
  const rest = whole % 60;
  return `${minutes}:${String(rest).padStart(2, "0")}`;
}

export function perMinute(count: number | null, secs: number): number | null {
  return count === null || secs <= 0 ? null : (count * 60) / secs;
}

export function liveElapsedSecs(run: ArbitrationRun, now: number): number {
  if (run.status !== "recording" || run.started_at === null) {
    return run.duration_secs;
  }

  return Math.max(
    run.duration_secs,
    (now - new Date(run.started_at).getTime()) / 1000,
  );
}

export function rateText(rate: number | null, digits: number): string {
  return rate === null ? "-" : rate.toFixed(digits);
}

export type ArbitrationRunMetric = "drone_rate" | "live_agents";

export const ARBITRATION_RUN_METRICS: Record<
  ArbitrationRunMetric,
  { label: string; digits: number; headline: "peak" | "latest" }
> = {
  drone_rate: { label: "Drones per minute", digits: 2, headline: "peak" },
  live_agents: { label: "Enemies alive", digits: 0, headline: "latest" },
};

export const ARBITRATION_RUN_METRIC_OPTIONS: FilterOption<ArbitrationRunMetric>[] =
  (["drone_rate", "live_agents"] as const).map((value) => ({
    value,
    label: ARBITRATION_RUN_METRICS[value].label,
  }));

export interface ArbitrationRunReading {
  secs: number;
  label: string;
  value: number | null;
}

export interface ArbitrationRunCycle {
  index: number;
  at_secs: number;
  duration_secs: number | null;
  drones: number | null;
}

export interface ArbitrationRunCyclePace {
  fastest: number;
  median: number;
  slowest: number;
}

function dronesAt(samples: ArbitrationRunSample[], secs: number): number {
  let count = 0;
  for (const sample of samples) {
    if (sample.secs > secs) {
      break;
    }
    count = sample.drone_spawns;
  }

  return count;
}

function droneRate(
  samples: ArbitrationRunSample[],
  secs: number,
): number | null {
  const from = Math.max(0, secs - RATE_WINDOW_SECS);
  const span = secs - from;
  if (span < RATE_MIN_SPAN_SECS) {
    return null;
  }

  return perMinute(dronesAt(samples, secs) - dronesAt(samples, from), span);
}

export function metricReadings(
  run: ArbitrationRun,
  metric: ArbitrationRunMetric,
): ArbitrationRunReading[] {
  return run.samples.map((sample) => ({
    secs: sample.secs,
    label: clockText(sample.secs),
    value:
      metric === "drone_rate"
        ? droneRate(run.samples, sample.secs)
        : sample.live_agents,
  }));
}

export function cycleRows(run: ArbitrationRun): ArbitrationRunCycle[] {
  const counted = run.samples.length > 0;
  let previous = 0;
  return run.rotations.map((rotation) => {
    const reached = dronesAt(run.samples, rotation.at_secs);
    const drones = counted ? reached - previous : null;
    previous = reached;
    return { ...rotation, drones };
  });
}

export function cyclePace(run: ArbitrationRun): ArbitrationRunCyclePace | null {
  const lengths = run.rotations
    .map((rotation) => rotation.duration_secs)
    .filter((secs): secs is number => secs !== null);
  if (lengths.length === 0) {
    return null;
  }

  return {
    fastest: Math.min(...lengths),
    median: median(lengths),
    slowest: Math.max(...lengths),
  };
}
