import {
  ChartHeader,
  ChartTooltip,
  useChartHover,
} from "@/components/chart-hover";
import {
  AXIS_TICK,
  HEIGHT,
  INNER_HEIGHT,
  labelledTicks,
  PADDING,
  TICK_BASELINE,
  tickWidth,
  tickX,
  WIDTH,
} from "@/lib/chart";
import { counted, monthLabel, spansYears } from "@/lib/format";
import { cn } from "@/lib/utils";

const INSET = 10;
const PLOT_WIDTH = WIDTH - INSET * 2;
const GAP = 2;
const MAX_TICKS = 4;
const ROW_CELL = 24;
const ROW_HEIGHT = 48;
const GRID_CELL = 14;
const SINGLE_ROW_LIMIT = 31;
const WEEK = 7;
const WIDE = 2.5;
const TALL = 2;
const DIM = 0.35;

export interface Day {
  key: string;
  label: string;
  played: boolean;
}

interface Layout {
  rows: number;
  offset: number;
  pitchX: number;
  pitchY: number;
  cellWidth: number;
  cellHeight: number;
  left: number;
  top: number;
}

function weekday(day: string): number {
  return new Date(`${day}T00:00:00Z`).getUTCDay();
}

function layoutOf(days: Day[]): Layout {
  const first = days[0];
  if (days.length <= SINGLE_ROW_LIMIT || !first) {
    const pitchX = Math.min(
      PLOT_WIDTH / Math.max(1, days.length),
      ROW_CELL + GAP,
    );
    return {
      rows: 1,
      offset: 0,
      pitchX,
      pitchY: 0,
      cellWidth: pitchX - GAP,
      cellHeight: ROW_HEIGHT,
      left: INSET + (PLOT_WIDTH - pitchX * days.length) / 2,
      top: PADDING.top + (INNER_HEIGHT - ROW_HEIGHT) / 2,
    };
  }
  const offset = weekday(first.key);
  const columns = Math.ceil((days.length + offset) / WEEK);
  const pitchX = Math.min(PLOT_WIDTH / columns, (GRID_CELL + GAP) * WIDE);
  const pitchY = Math.min(INNER_HEIGHT / WEEK, GRID_CELL + GAP, pitchX * TALL);
  return {
    rows: WEEK,
    offset,
    pitchX,
    pitchY,
    cellWidth: pitchX - GAP,
    cellHeight: pitchY - GAP,
    left: INSET + (PLOT_WIDTH - pitchX * columns) / 2,
    top: PADDING.top + (INNER_HEIGHT - pitchY * WEEK) / 2,
  };
}

interface Tick {
  key: string;
  text: string;
  centre: number;
}

function monthTicks(days: Day[], layout: Layout, withYear: boolean): Tick[] {
  const starts = days.flatMap((day, index) =>
    index === 0 || day.key.endsWith("-01")
      ? [
          {
            key: day.key,
            text: monthLabel(day.key, withYear),
            centre:
              layout.left +
              Math.floor((index + layout.offset) / WEEK) * layout.pitchX +
              layout.pitchX / 2,
          },
        ]
      : [],
  );
  const kept: Tick[] = [];
  for (const tick of starts) {
    const previous = kept.at(-1);
    const room = previous
      ? (tickWidth(previous.text) + tickWidth(tick.text)) / 2 + 8
      : 0;
    if (!previous || tick.centre - previous.centre >= room) {
      kept.push(tick);
    }
  }
  return kept;
}

function dayTicks(days: Day[], layout: Layout): Tick[] {
  return labelledTicks(days, (day) => day.label, layout.pitchX, MAX_TICKS).map(
    (tick) => ({
      key: tick.entry.key,
      text: tick.entry.label,
      centre: layout.left + tick.index * layout.pitchX + layout.pitchX / 2,
    }),
  );
}

export function DayStrip({
  label,
  color,
  days,
  format,
  className,
}: {
  label: string;
  color: string;
  days: Day[];
  format: (value: number) => string;
  className?: string;
}) {
  const played = days.filter((day) => day.played).length;
  const layout = layoutOf(days);
  const withYear = spansYears(days.map((day) => day.key));
  const ticks =
    layout.rows === WEEK
      ? monthTicks(days, layout, withYear)
      : dayTicks(days, layout);
  const columnOf = (index: number) =>
    Math.floor((index + layout.offset) / layout.rows);
  const hover = useChartHover(days.length, (x, y) => {
    const column = Math.floor((x - layout.left) / layout.pitchX);
    const row =
      layout.rows === 1 ? 0 : Math.floor((y - layout.top) / layout.pitchY);
    if (column < 0 || row < 0 || row >= layout.rows) {
      return null;
    }
    const index = column * layout.rows + row - layout.offset;
    return index < 0 || index >= days.length ? null : index;
  });
  const hovered = hover.index === null ? null : (days[hover.index] ?? null);

  return (
    <div className={cn("flex flex-col gap-2", className)}>
      <ChartHeader
        color={color}
        label={label}
        note={
          layout.rows === WEEK
            ? "one cell a day, a week a column"
            : "one cell a day"
        }
        value={format(played)}
        caption={
          days.length === 0
            ? "no days in this window"
            : `of ${counted(days.length, "day")} in this window`
        }
      />
      <div className="relative">
        <svg
          viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
          className="w-full"
          role="img"
          aria-label={`${label} as one shaded cell per day`}
          {...hover.handlers}
        >
          <title>{label}</title>
          {days.map((day, index) => (
            <rect
              key={day.key}
              x={layout.left + columnOf(index) * layout.pitchX + GAP / 2}
              y={
                layout.top +
                ((index + layout.offset) % layout.rows) * layout.pitchY
              }
              width={layout.cellWidth}
              height={layout.cellHeight}
              rx={Math.min(3, layout.cellWidth / 2)}
              fill={day.played ? color : "currentColor"}
              fillOpacity={
                day.played
                  ? hover.index === null || hover.index === index
                    ? 1
                    : DIM
                  : 0.12
              }
            />
          ))}
          {ticks.map((tick) => (
            <text
              key={tick.key}
              x={tickX(tick.centre, tick.text)}
              y={TICK_BASELINE}
              textAnchor="middle"
              {...AXIS_TICK}
            >
              {tick.text}
            </text>
          ))}
        </svg>
        {hovered !== null && hover.index !== null && (
          <ChartTooltip
            at={layout.left + columnOf(hover.index) * layout.pitchX}
            label={hovered.label}
            readings={[
              {
                key: "played",
                color: hovered.played ? color : "var(--muted-foreground)",
                text: hovered.played ? "played" : "not played",
              },
            ]}
          />
        )}
      </div>
    </div>
  );
}
