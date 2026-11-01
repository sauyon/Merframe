import { useEffect, useState } from "react";
import type { BuffTimer } from "@/types";

// Sized to sit just outside the game's reticle and read like its HUD: thin white strokes,
// small seconds left beside each arc, colour only as a hint when something needs recasting.
// Abilities keep fixed slots, so where an arc sits says which ability it is.
const SIZE = 440;
const CENTRE = SIZE / 2;
const RADIUS = 42;
const THICKNESS = 1.2;
const GAP = 0.12;
/** Ability slots along the bottom of the ring. */
const SLOTS = 4;
/** How far around the ring the bottom slots, and the other buffs on top, each reach. */
const SPREAD = (5 * Math.PI) / 6;
const LABEL_OFFSET = 9;
/** The arc warms once this share of the buff is left. */
const WARN_SHARE = 0.25;
const INK = "rgb(235 238 240)";
const WARN = "rgb(240 190 110)";
const DOWN = "rgb(235 110 95)";

function useNow() {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    let frame = 0;
    const step = () => {
      setNow(Date.now());
      frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, []);
  return now;
}

function point(angle: number, radius: number) {
  return [
    CENTRE + radius * Math.sin(angle),
    CENTRE - radius * Math.cos(angle),
  ] as const;
}

/** An arc clockwise from `from` to `to`, in radians from straight up. */
function arc(from: number, to: number) {
  if (to - from <= 0.001) {
    return "";
  }
  const [x0, y0] = point(from, RADIUS);
  const [x1, y1] = point(to, RADIUS);
  const large = to - from > Math.PI ? 1 : 0;
  return `M ${x0} ${y0} A ${RADIUS} ${RADIUS} 0 ${large} 1 ${x1} ${y1}`;
}

function Buff({
  buff,
  from,
  to,
  inside,
  now,
}: {
  buff: BuffTimer;
  from: number;
  to: number;
  inside: boolean;
  now: number;
}) {
  const left =
    buff.ends_at === null ? null : Math.max(0, (buff.ends_at - now) / 1000);
  const share =
    left === null ? 0 : Math.min(1, left / Math.max(buff.total, 0.001));
  const colour = left === null ? DOWN : share < WARN_SHARE ? WARN : INK;
  const [x, y] = point(
    (from + to) / 2,
    RADIUS + (inside ? -LABEL_OFFSET : LABEL_OFFSET),
  );

  return (
    <g>
      {left === null && (
        <path
          d={arc(from, to)}
          stroke={DOWN}
          strokeOpacity={0.5}
          strokeWidth={THICKNESS}
          fill="none"
        />
      )}
      <path
        d={arc(from, from + (to - from) * share)}
        stroke={colour}
        strokeOpacity={0.85}
        strokeWidth={THICKNESS}
        fill="none"
      />
      <text
        x={x}
        y={y}
        fill={colour}
        fillOpacity={0.9}
        fontWeight={600}
        textAnchor="middle"
        dominantBaseline="middle"
        fontSize={7}
        style={{
          filter: "drop-shadow(0 0 0.6px rgb(0 0 0 / 0.9))",
        }}
      >
        {left === null ? "—" : Math.ceil(left)}
      </text>
    </g>
  );
}

/** Of several buffs from one ability, the running one that ends first. */
function pick(buffs: BuffTimer[]) {
  const end = (buff: BuffTimer) => buff.ends_at ?? Number.POSITIVE_INFINITY;
  return buffs.reduce((best, buff) => (end(buff) < end(best) ? buff : best));
}

/** `count` equal arcs across `spread` radians centred on `centre`, in clockwise order. */
function sections(centre: number, spread: number, count: number) {
  const span = spread / count;
  return Array.from({ length: count }, (_, index) => {
    const from = centre - spread / 2 + index * span + GAP / 2;
    return [from, from + span - GAP] as const;
  });
}

export function BuffRing({
  buffs,
  opacity,
}: {
  buffs: BuffTimer[];
  opacity: number;
}) {
  const now = useNow();
  // Abilities 1 to 4 sit along the bottom, left to right like the game's ability dots.
  // Clockwise around the bottom runs right to left, so slot 4 comes first.
  const slots = sections(Math.PI, SPREAD, SLOTS)
    .reverse()
    .map(([from, to], index) => ({
      from,
      to,
      buffs: buffs.filter((buff) => buff.slot === index + 1),
    }));
  const others = buffs.filter(
    (buff) => buff.slot === null || buff.slot > SLOTS,
  );
  const top = sections(0, SPREAD, Math.max(others.length, 1));

  return (
    <svg
      viewBox={`0 0 ${SIZE} ${SIZE}`}
      className="size-full"
      style={{ opacity: opacity / 100 }}
    >
      {slots.map(({ from, to, buffs }) => (
        <g key={from}>
          <path
            d={arc(from, to)}
            stroke={INK}
            strokeOpacity={0.14}
            strokeWidth={THICKNESS}
            fill="none"
          />
          {buffs.length > 0 && (
            <Buff buff={pick(buffs)} from={from} to={to} inside now={now} />
          )}
        </g>
      ))}
      {others.map((buff, index) => {
        const [from, to] = top[index] ?? [0, 0];
        return (
          <g key={buff.id}>
            <path
              d={arc(from, to)}
              stroke={INK}
              strokeOpacity={0.14}
              strokeWidth={THICKNESS}
              fill="none"
            />
            <Buff buff={buff} from={from} to={to} inside={false} now={now} />
          </g>
        );
      })}
    </svg>
  );
}
