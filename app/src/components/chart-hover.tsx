import {
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
  useState,
} from "react";
import type { TooltipContentProps } from "recharts";
import { Hint } from "@/components/ui/hint";
import { HEIGHT, WIDTH } from "@/lib/chart";
import { cn } from "@/lib/utils";

export interface Reading {
  key: string;
  color: string;
  text: string;
  note?: string;
}

export function useChartHover(
  count: number,
  locate: (x: number, y: number) => number | null,
) {
  const [index, setIndex] = useState<number | null>(null);
  const move = (step: number) =>
    setIndex((current) =>
      Math.min(count - 1, Math.max(0, (current ?? count) + step)),
    );

  return {
    index: index !== null && index < count ? index : null,
    handlers: {
      tabIndex: count === 0 ? undefined : 0,
      onPointerMove: (e: PointerEvent<SVGSVGElement>) => {
        const box = e.currentTarget.getBoundingClientRect();
        setIndex(
          locate(
            ((e.clientX - box.left) / box.width) * WIDTH,
            ((e.clientY - box.top) / box.height) * HEIGHT,
          ),
        );
      },
      onPointerLeave: () => setIndex(null),
      onFocus: () => {
        if (count > 0) {
          setIndex((current) => current ?? count - 1);
        }
      },
      onBlur: () => setIndex(null),
      onKeyDown: (e: KeyboardEvent<SVGSVGElement>) => {
        if (count === 0) {
          return;
        }
        if (e.key === "ArrowLeft") {
          move(-1);
        } else if (e.key === "ArrowRight") {
          move(1);
        } else {
          return;
        }
        e.preventDefault();
      },
    },
  };
}

export function ChartHeader({
  color,
  label,
  note,
  value,
  caption,
  control,
}: {
  color: string;
  label: string;
  note?: ReactNode;
  value: ReactNode;
  caption: ReactNode;
  control?: ReactNode;
}) {
  return (
    <>
      <div
        className={cn(
          "flex justify-between gap-2",
          control ? "items-center" : "items-baseline",
        )}
      >
        <Hint as="span" className="flex items-center gap-1.5">
          <span
            className="size-2 rounded-full"
            style={{ background: color }}
            aria-hidden="true"
          />
          {label}
        </Hint>
        {(note || control) && (
          <span className="flex items-center gap-2">
            {note && <Hint as="span">{note}</Hint>}
            {control}
          </span>
        )}
      </div>
      <div className="flex items-baseline gap-2">
        <span className="text-foreground text-lg font-bold">{value}</span>
        <Hint as="span">{caption}</Hint>
      </div>
    </>
  );
}

export function ChartReadings({
  label,
  readings,
  className,
}: {
  label: string;
  readings: Reading[];
  className?: string;
}) {
  return (
    <div
      className={cn(
        "bg-popover text-popover-foreground rounded-md border px-2 py-1.5 shadow-md",
        className,
      )}
    >
      <span className="text-muted-foreground block text-xs">{label}</span>
      {readings.map((reading) => (
        <span key={reading.key} className="flex items-center gap-1.5 text-xs">
          <span
            className="h-0.5 w-2.5 rounded-full"
            style={{ background: reading.color }}
            aria-hidden="true"
          />
          <span className="text-foreground font-bold">{reading.text}</span>
          {reading.note && (
            <span className="text-muted-foreground">{reading.note}</span>
          )}
        </span>
      ))}
    </div>
  );
}

export function readingsTooltip<T extends { label: string }>(
  readings: (entry: T) => Reading[],
) {
  return ({ active, payload }: TooltipContentProps) => {
    const entry: T | undefined = payload[0]?.payload;
    return active && entry ? (
      <ChartReadings label={entry.label} readings={readings(entry)} />
    ) : null;
  };
}

export function ChartTooltip({
  at,
  label,
  readings,
}: {
  at: number;
  label: string;
  readings: Reading[];
}) {
  return (
    <ChartReadings
      label={label}
      readings={readings}
      className={cn(
        "pointer-events-none absolute top-0 z-10",
        at > WIDTH / 2 ? "left-0" : "right-0",
      )}
    />
  );
}
