export const WIDTH = 360;
export const HEIGHT = 152;
export const PADDING = { top: 12, right: 10, bottom: 22, left: 46 };
export const INNER_HEIGHT = HEIGHT - PADDING.top - PADDING.bottom;

export const TICK_FONT = 10;
export const TICK_BASELINE = HEIGHT - 6;

export const AXIS_TICK = {
  fontSize: TICK_FONT,
  fill: "currentColor",
  fillOpacity: 0.6,
};
export const GRID = { stroke: "currentColor", strokeOpacity: 0.1 };
export const MARGIN = { top: 8, right: 8, bottom: 0, left: 0 };
export const LABELLED_MARGIN = { ...MARGIN, top: 16 };
export const LINE_CURSOR = { stroke: "currentColor", strokeOpacity: 0.3 };
export const BAR_CURSOR = { fill: "currentColor", fillOpacity: 0.08 };

export function changeTone(change: number): string | undefined {
  if (change === 0) {
    return undefined;
  }
  return change > 0 ? "text-chart-gain" : "text-chart-loss";
}

export function changeColor(change: number): string {
  return change < 0 ? "var(--chart-loss)" : "var(--chart-gain)";
}

export function signed(
  value: number,
  format: (value: number) => string,
): string {
  return `${value > 0 ? "+" : ""}${format(value)}`;
}

const GLYPH_WIDTH = 0.6;
const TICK_CLEARANCE = 10;

export const COMPACT = new Intl.NumberFormat("en-US", {
  notation: "compact",
  maximumFractionDigits: 1,
});

export function tickWidth(text: string): number {
  return text.length * TICK_FONT * GLYPH_WIDTH;
}

export function labelledTicks<T>(
  entries: T[],
  label: (entry: T) => string,
  pitch: number,
  maximum: number,
): { entry: T; index: number }[] {
  const last = entries.length - 1;
  if (last < 0) {
    return [];
  }
  const widest = Math.max(...entries.map((entry) => tickWidth(label(entry))));
  const stride = Math.max(
    1,
    Math.ceil(entries.length / maximum),
    Math.ceil((widest + TICK_CLEARANCE) / pitch),
  );
  return entries
    .map((entry, index) => ({ entry, index }))
    .filter(
      (tick) =>
        tick.index === last ||
        (tick.index % stride === 0 && last - tick.index >= stride),
    );
}

export function tickX(center: number, text: string): number {
  const half = tickWidth(text) / 2;
  return Math.min(Math.max(center, half + 2), WIDTH - half - 2);
}
