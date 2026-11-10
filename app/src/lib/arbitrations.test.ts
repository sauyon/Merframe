import { describe, expect, test } from "vitest";
import type { ArbitrationSchedule, ArbitrationSlot } from "@/types";
import {
  currentSlot,
  dayKey,
  dayName,
  groupByDay,
  liveSlots,
  nextMatch,
  nextSlot,
  nodeValues,
  personalMatch,
  slotMission,
  slotPlace,
  slotSearchText,
  upcomingSlot,
} from "./arbitrations";

const HOUR = 3_600_000;
const FIRST = Date.UTC(2026, 9, 24, 12);

function slot(
  hour: number,
  extra: Partial<ArbitrationSlot> = {},
): ArbitrationSlot {
  const starts = FIRST + hour * HOUR;
  return {
    id: `${starts / 1000}:SolNode64`,
    starts: new Date(starts).toISOString(),
    ends: new Date(starts + HOUR).toISOString(),
    favourite: false,
    matches: true,
    reminder: false,
    node_id: "SolNode64",
    node: "Umbriel",
    planet: "Uranus",
    mission_type: "Interception",
    faction: "Grineer",
    tier: "S",
    ...extra,
  };
}

function schedule(
  slots: ArbitrationSlot[],
  has_match_rules = false,
): ArbitrationSchedule {
  return { slots, nodes: [], has_match_rules };
}

describe("slot selection", () => {
  const slots = [slot(0), slot(1, { matches: false }), slot(2)];
  const now = FIRST + 30 * 60_000;

  test("live, current and next", () => {
    expect(liveSlots(schedule(slots), FIRST + HOUR)).toHaveLength(2);
    expect(currentSlot(slots, FIRST + HOUR)?.id).toBe(slots[1]?.id);
    expect(liveSlots(null, now)).toEqual([]);
    expect(currentSlot(slots, now)?.id).toBe(slots[0]?.id);
    expect(currentSlot(slots, FIRST - 1)).toBeNull();
    expect(nextSlot(slots, now)?.id).toBe(slots[1]?.id);
    expect(nextSlot(slots, FIRST + 2 * HOUR)).toBeNull();
  });

  test("next match needs rules and skips non-matching hours", () => {
    expect(nextMatch(schedule(slots), now)).toBeNull();
    expect(nextMatch(schedule(slots, true), now)?.id).toBe(slots[2]?.id);
    expect(nextMatch(schedule([slot(0)], true), now)).toBeNull();
    expect(upcomingSlot(schedule(slots), now)).toEqual({
      slot: slots[1],
      label: "Next",
    });
    expect(upcomingSlot(schedule(slots, true), now)).toEqual({
      slot: slots[2],
      label: "Next match",
    });
    expect(upcomingSlot(schedule([slot(0), slot(1)], true), now)).toEqual({
      slot: slot(1),
      label: "Next",
    });
    expect(upcomingSlot(null, now)).toBeNull();
  });
});

test("node values are distinct and sorted", () => {
  const nodes = [
    { ...slot(0), planet: "Uranus" },
    { ...slot(0), planet: "Mars" },
    { ...slot(0), planet: null },
    { ...slot(0), planet: "Uranus" },
  ];
  expect(nodeValues(nodes, "planet")).toEqual(["Mars", "Uranus"]);
});

test("personal filters read the slot marks", () => {
  const marked = slot(0, { favourite: true, reminder: true, matches: false });
  expect(personalMatch(marked, null)).toBe(true);
  expect(personalMatch(marked, "favourites")).toBe(true);
  expect(personalMatch(marked, "reminders")).toBe(true);
  expect(personalMatch(marked, "matches")).toBe(false);
});

test("labels and search text", () => {
  expect(slotPlace(slot(0))).toBe("Umbriel, Uranus");
  expect(slotMission(slot(0, { mission_type: null }))).toBe("Unknown mission");
  expect(slotPlace({ node: "SolNode99999", planet: null })).toBe(
    "SolNode99999",
  );
  expect(slotPlace({ node: null, planet: null })).toBe("Unknown node");
  expect(slotSearchText(slot(0))).toBe("umbriel uranus interception grineer s");
  expect(slotSearchText(slot(0, { planet: null, faction: null }))).toBe(
    "umbriel interception s",
  );
});

test("days are grouped in local time", () => {
  const now = FIRST;
  const key = dayKey(now);
  expect(key).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  expect(dayName(now, now)).toBe("Today");
  expect(dayName(now + 24 * HOUR, now)).toBe("Tomorrow");
  const clockChange = new Date(2026, 9, 25, 12).getTime();
  const nextNoon = new Date(2026, 9, 26, 12).getTime();
  expect(dayName(nextNoon, clockChange)).toBe("Tomorrow");
  expect(dayName(now + 48 * HOUR, now)).toHaveLength(3);
  expect(groupByDay([], now)).toEqual([]);
  const days = groupByDay([slot(0), slot(1), slot(24), slot(49)], now);
  expect(days.map((day) => day.slots.length)).toEqual([2, 1, 1]);
  expect(days.map((day) => day.label).slice(0, 2)).toEqual([
    "Today",
    "Tomorrow",
  ]);
  expect(days[0]?.date).toBe("24 Oct 2026");
});
