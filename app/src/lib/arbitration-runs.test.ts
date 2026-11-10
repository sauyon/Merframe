import { expect, test } from "vitest";
import type { ArbitrationRun } from "@/types";
import {
  boosterText,
  clockText,
  cyclePace,
  cycleRows,
  earningsSummary,
  elapsedText,
  liveElapsedSecs,
  metricReadings,
  perMinute,
  rateText,
  runRole,
  runTitle,
} from "./arbitration-runs";

function run(extra: Partial<ArbitrationRun> = {}): ArbitrationRun {
  return {
    id: "1792858804954:16468.857",
    source: "live",
    status: "completed",
    node_id: "ClanNode11",
    node: "Zeugma",
    planet: "Phobos",
    mission_type: "Survival",
    started_at: "2026-10-24T20:54:38Z",
    duration_secs: 333.9,
    host: true,
    complete_capture: true,
    notes: [],
    rotations: [{ index: 1, at_secs: 313.2, duration_secs: 300.5 }],
    waves: [],
    reward_cycles: 1,
    enemy_spawns: 420,
    drone_spawns: 7,
    peak_live_agents: 18,
    boosters: [],
    samples: [
      { secs: 5, drone_spawns: 1, live_agents: 5 },
      { secs: 10, drone_spawns: 1, live_agents: 8 },
      { secs: 15, drone_spawns: 2, live_agents: 9 },
    ],
    rewards: null,
    collected_vitus: null,
    manual_vitus: null,
    vitus: null,
    ...extra,
  };
}

test("earnings summary", () => {
  expect(earningsSummary([run()])).toEqual({
    runs: 0,
    vitus: 0,
    perHour: null,
  });
  const summary = earningsSummary([
    run({ vitus: 3, duration_secs: 600 }),
    run({ vitus: 9, duration_secs: 1200 }),
    run({ vitus: 4, duration_secs: 0 }),
  ]);
  expect(summary).toEqual({ runs: 3, vitus: 16, perHour: 24 });
});

test("rate text", () => {
  expect(rateText(null, 2)).toBe("-");
  expect(rateText(1.2567, 2)).toBe("1.26");
  expect(rateText(75.5, 1)).toBe("75.5");
});

test("labels", () => {
  expect(runTitle(run())).toBe("Survival, Zeugma, Phobos");
  expect(runRole(run())).toBe("Host");
  expect(runRole(run({ host: false }))).toBe("Client");
  expect(runRole(run({ host: null }))).toBe("Role unknown");
  expect(elapsedText(333.9)).toBe("5m 33s");
  expect(elapsedText(0)).toBe("0s");
  expect(elapsedText(0.5)).toBe("0s");
  expect(clockText(65.7)).toBe("1:05");
  expect(perMinute(7, 333.9)).toBeCloseTo(1.258, 3);
  expect(perMinute(7, 0)).toBeNull();
});

test("booster text", () => {
  const blessing = { kind: "drop_chance_blessing", expires_secs: 200 } as const;
  expect(boosterText(blessing, 150)).toBe("Resource Drop Chance Blessing");
  expect(boosterText(blessing, 333.9)).toBe(
    "Resource Drop Chance Blessing, ran out at 3m 20s",
  );
  expect(boosterText({ kind: "amount", expires_secs: 7200 }, 333.9)).toBe(
    "Resource Booster",
  );
});

test("live elapsed", () => {
  const started = new Date("2026-10-24T20:54:38Z").getTime();
  const live = { status: "recording" } as const;
  expect(liveElapsedSecs(run(live), started + 400_000)).toBe(400);
  expect(liveElapsedSecs(run(live), started + 100_000)).toBe(333.9);
  expect(liveElapsedSecs(run({ ...live, started_at: null }), started)).toBe(
    333.9,
  );
  expect(liveElapsedSecs(run(), started + 400_000)).toBe(333.9);
});

test("metric readings", () => {
  const readings = metricReadings(run(), "live_agents");
  expect(readings.map((reading) => reading.label)).toEqual([
    "0:05",
    "0:10",
    "0:15",
  ]);
  expect(readings.map((reading) => reading.value)).toEqual([5, 8, 9]);
});

test("drone rate over a trailing minute", () => {
  const samples = [
    { secs: 10, drone_spawns: 1, live_agents: 5 },
    { secs: 40, drone_spawns: 2, live_agents: 5 },
    { secs: 90, drone_spawns: 4, live_agents: 5 },
    { secs: 130, drone_spawns: 5, live_agents: 5 },
  ];
  const rates = metricReadings(run({ samples }), "drone_rate").map(
    (reading) => reading.value,
  );
  expect(rates[0]).toBeNull();
  expect(rates.slice(1)).toEqual([3, 3, 3]);
});

test("drones per cycle", () => {
  const rotations = [
    { index: 1, at_secs: 12, duration_secs: null },
    { index: 2, at_secs: 20, duration_secs: 8 },
  ];
  expect(cycleRows(run({ rotations })).map((cycle) => cycle.drones)).toEqual([
    1, 1,
  ]);
  expect(
    cycleRows(run({ rotations, samples: [] })).map((cycle) => cycle.drones),
  ).toEqual([null, null]);
});

test("cycle pace", () => {
  expect(cyclePace(run())).toEqual({
    fastest: 300.5,
    median: 300.5,
    slowest: 300.5,
  });
  const rotations = [
    { index: 1, at_secs: 100, duration_secs: null },
    { index: 2, at_secs: 220, duration_secs: 120 },
    { index: 3, at_secs: 380, duration_secs: 160 },
    { index: 4, at_secs: 530, duration_secs: 150 },
    { index: 5, at_secs: 740, duration_secs: 210 },
  ];
  expect(cyclePace(run({ rotations }))).toEqual({
    fastest: 120,
    median: 155,
    slowest: 210,
  });
  expect(cyclePace(run({ rotations: rotations.slice(0, 4) }))).toEqual({
    fastest: 120,
    median: 150,
    slowest: 160,
  });
  expect(cyclePace(run({ rotations: [] }))).toBeNull();
});
