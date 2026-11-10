import type { EventName } from "@/lib/bridge";
import { fullDate } from "@/lib/format";
import type {
  ArbitrationSchedule,
  ArbitrationSlot,
  ArbitrationTier,
} from "@/types";

export const ARBITRATION_ALERTS_SECTION_ID = "arbitration-alerts";
export const ARBITRATION_SETTINGS_PATH = `/settings#${ARBITRATION_ALERTS_SECTION_ID}`;

export const ARBITRATION_RELOAD_EVENTS: EventName[] = [
  "worldStateUpdated",
  "settingsUpdated",
];

export const ARBITRATION_TIERS: ArbitrationTier[] = [
  "S",
  "A",
  "B",
  "C",
  "D",
  "F",
];

export const ARBITRATION_LEAD_MINUTES = [0, 5, 10, 15, 30, 60] as const;

export type ArbitrationPersonalFilter = "favourites" | "matches" | "reminders";

export type ArbitrationRunRoleFilter = "host" | "client";

export function slotPlace(slot: {
  node: string | null;
  planet: string | null;
}): string {
  const node = slot.node ?? "Unknown node";
  return slot.planet ? `${node}, ${slot.planet}` : node;
}

export function slotMission(slot: { mission_type: string | null }): string {
  return slot.mission_type ?? "Unknown mission";
}

export function nodeValues<K extends string>(
  nodes: Record<K, string | null>[],
  key: K,
): string[] {
  const known = nodes
    .map((node) => node[key])
    .filter((value) => value !== null);
  return [...new Set(known)].sort((a, b) => a.localeCompare(b));
}

function startsAt(slot: ArbitrationSlot): number {
  return new Date(slot.starts).getTime();
}

function endsAt(slot: ArbitrationSlot): number {
  return new Date(slot.ends).getTime();
}

export function liveSlots(
  schedule: ArbitrationSchedule | null,
  now: number,
): ArbitrationSlot[] {
  return (schedule?.slots ?? []).filter((slot) => endsAt(slot) > now);
}

export function currentSlot(
  slots: ArbitrationSlot[],
  now: number,
): ArbitrationSlot | null {
  return (
    slots.find((slot) => startsAt(slot) <= now && endsAt(slot) > now) ?? null
  );
}

export function nextSlot(
  slots: ArbitrationSlot[],
  now: number,
): ArbitrationSlot | null {
  return slots.find((slot) => startsAt(slot) > now) ?? null;
}

export function nextMatch(
  schedule: ArbitrationSchedule | null,
  now: number,
): ArbitrationSlot | null {
  if (!schedule?.has_match_rules) {
    return null;
  }

  return (
    schedule.slots.find((slot) => startsAt(slot) > now && slot.matches) ?? null
  );
}

export function upcomingSlot(
  schedule: ArbitrationSchedule | null,
  now: number,
): { slot: ArbitrationSlot; label: "Next" | "Next match" } | null {
  const slots = liveSlots(schedule, now);
  const next = nextSlot(slots, now);
  const match = nextMatch(schedule, now);
  if (match && next && match.id !== next.id) {
    return { slot: match, label: "Next match" };
  }

  return next ? { slot: next, label: "Next" } : null;
}

export function personalMatch(
  slot: ArbitrationSlot,
  personal: ArbitrationPersonalFilter | null,
): boolean {
  switch (personal) {
    case "favourites":
      return slot.favourite;
    case "matches":
      return slot.matches;
    case "reminders":
      return slot.reminder;
    case null:
      return true;
  }
}

export function slotSearchText(slot: ArbitrationSlot): string {
  return [slot.node, slot.planet, slot.mission_type, slot.faction, slot.tier]
    .filter((part) => part !== null)
    .join(" ")
    .toLocaleLowerCase();
}

export function dayKey(at: number): string {
  const date = new Date(at);
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

function tomorrowKey(now: number): string {
  const tomorrow = new Date(now);
  tomorrow.setDate(tomorrow.getDate() + 1);
  return dayKey(tomorrow.getTime());
}

export function dayName(at: number, now: number): string {
  const key = dayKey(at);
  if (key === dayKey(now)) {
    return "Today";
  }

  if (key === tomorrowKey(now)) {
    return "Tomorrow";
  }

  return new Date(at).toLocaleDateString("en-GB", { weekday: "short" });
}

export interface ArbitrationDay {
  key: string;
  label: string;
  date: string;
  slots: ArbitrationSlot[];
}

export function groupByDay(
  slots: ArbitrationSlot[],
  now: number,
): ArbitrationDay[] {
  const days: ArbitrationDay[] = [];
  for (const slot of slots) {
    const at = startsAt(slot);
    const key = dayKey(at);
    const day = days.find((day) => day.key === key);
    if (day) {
      day.slots.push(slot);
    } else {
      days.push({
        key,
        label: dayName(at, now),
        date: fullDate(slot.starts),
        slots: [slot],
      });
    }
  }

  return days;
}
