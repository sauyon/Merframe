import { Bell, type LucideIcon, Star, Timer } from "lucide-react";
import type { ReactNode } from "react";
import { useNavigate } from "react-router";
import { Figure } from "@/components/figure";
import { GameIcon } from "@/components/game-icon";
import { MarkToggle } from "@/components/mark-toggle";
import { EmptyNote, FramedList, Surface } from "@/components/page";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Hint } from "@/components/ui/hint";
import { Progress } from "@/components/ui/progress";
import {
  ARBITRATION_SETTINGS_PATH,
  type ArbitrationDay,
  slotMission,
  slotPlace,
} from "@/lib/arbitrations";
import { secondsUntil, timeOfDay } from "@/lib/format";
import { cn } from "@/lib/utils";
import { countdownTone, elapsedShare, timeLeft } from "@/lib/world";
import type { ArbitrationSlot, ArbitrationTier } from "@/types";

const TIER_VARIANT: Record<ArbitrationTier, "accent" | "secondary" | "muted"> =
  {
    S: "accent",
    A: "accent",
    B: "secondary",
    C: "secondary",
    D: "muted",
    F: "muted",
  };

export function TierBadge({ tier }: { tier: ArbitrationTier }) {
  return (
    <Badge
      variant={TIER_VARIANT[tier]}
      className="w-8 justify-center"
      title={`Community tier ${tier}`}
    >
      {tier}
    </Badge>
  );
}

export interface SlotMarks {
  busy: boolean;
  reminderAllowed: boolean;
  onFavourite: (nodeId: string) => void;
  onReminder: (slot: ArbitrationSlot) => void;
}

function StarButton({
  slot,
  marks,
}: {
  slot: ArbitrationSlot;
  marks: SlotMarks;
}) {
  return (
    <MarkToggle
      icon={Star}
      pressed={slot.favourite}
      label={slot.favourite ? "Remove favourite node" : "Favourite this node"}
      disabled={marks.busy}
      onClick={() => marks.onFavourite(slot.node_id)}
    />
  );
}

function bellLabel(
  slot: ArbitrationSlot,
  marks: SlotMarks,
  started: boolean,
): string {
  if (started) {
    return "This rotation has already started";
  }

  if (slot.reminder) {
    return "Remove reminder";
  }

  return marks.reminderAllowed
    ? "Remind me about this rotation"
    : "Turn on a notification channel in Settings first";
}

function BellButton({
  slot,
  marks,
  started,
}: {
  slot: ArbitrationSlot;
  marks: SlotMarks;
  started: boolean;
}) {
  const disabled =
    marks.busy || started || (!marks.reminderAllowed && !slot.reminder);
  return (
    <MarkToggle
      icon={Bell}
      pressed={slot.reminder}
      label={bellLabel(slot, marks, started)}
      disabled={disabled}
      onClick={() => marks.onReminder(slot)}
    />
  );
}

export function HeroHeading({
  icon: Icon,
  label,
  children,
}: {
  icon: LucideIcon | "arbitration";
  label: string;
  children?: ReactNode;
}) {
  return (
    <div className="flex items-center gap-2">
      {Icon === "arbitration" ? (
        <GameIcon name="arbitration" size={16} alt="" />
      ) : (
        <Icon className="text-primary size-4" />
      )}
      <Hint as="span" className="flex-1 font-medium">
        {label}
      </Hint>
      {children}
    </div>
  );
}

function HeroCard({
  icon,
  label,
  slot,
  empty,
  mark,
  children,
}: {
  icon: LucideIcon | "arbitration";
  label: string;
  slot: ArbitrationSlot | null;
  empty: { title: string; note: string };
  mark?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <Surface className="gap-1 px-4 py-3">
      <HeroHeading icon={icon} label={label}>
        {slot && <TierBadge tier={slot.tier} />}
        {mark}
      </HeroHeading>
      {slot ? (
        <>
          <span className="text-primary truncate text-lg font-bold">
            {slotMission(slot)}
          </span>
          <span className="truncate text-sm">{slotPlace(slot)}</span>
          <Hint as="span">{slot.faction ?? "Unknown faction"}</Hint>
        </>
      ) : (
        <>
          <span className="text-lg font-bold">{empty.title}</span>
          <Hint as="span">{empty.note}</Hint>
        </>
      )}
      {children && (
        <div className="mt-auto flex flex-col gap-1 pt-2">{children}</div>
      )}
    </Surface>
  );
}

function CountdownFigure({
  label,
  seconds,
}: {
  label: string;
  seconds: number;
}) {
  return (
    <Figure
      label={label}
      value={timeLeft(seconds)}
      className={countdownTone(seconds)}
    />
  );
}

export function HeroCards({
  current,
  next,
  match,
  hasMatchRules,
  now,
  marks,
}: {
  current: ArbitrationSlot | null;
  next: ArbitrationSlot | null;
  match: ArbitrationSlot | null;
  hasMatchRules: boolean;
  now: number;
  marks: SlotMarks;
}) {
  const navigate = useNavigate();
  return (
    <div className="grid gap-3 @3xl:grid-cols-3">
      <HeroCard
        icon="arbitration"
        label="Current rotation"
        slot={current}
        empty={{
          title: "No current forecast",
          note: "The schedule refreshes on its own.",
        }}
        mark={current && <StarButton slot={current} marks={marks} />}
      >
        {current && (
          <>
            <CountdownFigure
              label="Time left to enter"
              seconds={secondsUntil(current.ends, now)}
            />
            <Progress value={elapsedShare(current.starts, current.ends, now)} />
          </>
        )}
      </HeroCard>
      <HeroCard
        icon={Timer}
        label="Up next"
        slot={next}
        empty={{
          title: "Next hour unknown",
          note: "The forecast has no later rotation.",
        }}
        mark={next && <BellButton slot={next} marks={marks} started={false} />}
      >
        {next && (
          <Figure
            label="Starts at"
            value={timeOfDay(next.starts)}
            className="text-accent"
          />
        )}
      </HeroCard>
      <HeroCard
        icon={Star}
        label="Next match"
        slot={match}
        empty={
          hasMatchRules
            ? {
                title: "No matching rotation",
                note: "Nothing in the next seven days matches your Arbitration filters.",
              }
            : {
                title: "No filters active",
                note: "Turn on matching Arbitration alerts and pick mission types, planets, factions, tiers or nodes under Settings.",
              }
        }
      >
        <div className="flex items-end justify-between gap-3">
          {match ? (
            <CountdownFigure
              label={`Starts at ${timeOfDay(match.starts)}`}
              seconds={secondsUntil(match.starts, now)}
            />
          ) : (
            <span />
          )}
          <Button
            variant="outline"
            size="sm"
            onClick={() => navigate(ARBITRATION_SETTINGS_PATH)}
          >
            Edit filters
          </Button>
        </div>
      </HeroCard>
    </div>
  );
}

function SlotRow({
  slot,
  now,
  marks,
}: {
  slot: ArbitrationSlot;
  now: number;
  marks: SlotMarks;
}) {
  const started = new Date(slot.starts).getTime() <= now;
  return (
    <li
      className={cn(
        "flex items-center gap-3 px-3 py-2",
        started && "bg-secondary/40",
      )}
    >
      <span className="flex w-24 shrink-0 flex-col font-mono text-sm">
        <span className="font-semibold">{timeOfDay(slot.starts)}</span>
        {started ? (
          <Hint as="span" className="text-accent">
            Current
          </Hint>
        ) : (
          <Hint as="span">to {timeOfDay(slot.ends)}</Hint>
        )}
      </span>
      <TierBadge tier={slot.tier} />
      <span className="min-w-0 flex-1 truncate text-sm">
        <span className="font-semibold">{slotMission(slot)}</span>
        <span className="text-muted-foreground"> ({slotPlace(slot)})</span>
      </span>
      <Hint as="span" className="hidden w-24 shrink-0 @md:block">
        {slot.faction ?? "Unknown faction"}
      </Hint>
      <StarButton slot={slot} marks={marks} />
      <BellButton slot={slot} marks={marks} started={started} />
    </li>
  );
}

export function DayList({
  days,
  total,
  now,
  marks,
  onClear,
}: {
  days: ArbitrationDay[];
  total: number;
  now: number;
  marks: SlotMarks;
  onClear: () => void;
}) {
  if (total === 0) {
    return <EmptyNote>The forecast is not loaded yet.</EmptyNote>;
  }

  if (days.length === 0) {
    return (
      <div className="flex flex-col items-start gap-3">
        <EmptyNote>No rotation matches these choices.</EmptyNote>
        <Button variant="outline" size="sm" onClick={onClear}>
          Clear search and filters
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {days.map((day) => (
        <FramedList
          key={day.key}
          header={
            <>
              <span className="text-sm font-semibold">{day.label}</span>
              <Hint as="span">{day.date}</Hint>
            </>
          }
        >
          {day.slots.map((slot) => (
            <SlotRow key={slot.id} slot={slot} now={now} marks={marks} />
          ))}
        </FramedList>
      ))}
    </div>
  );
}
