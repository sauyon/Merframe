import { useEffect, useState } from "react";
import type { BuffTimer } from "@/types";

// Sized to sit just outside the game's reticle and read like its HUD: thin white strokes,
// small uppercase text, colour only as a hint when something needs recasting.
const SIZE = 440;
const CENTRE = SIZE / 2;
const RADIUS = 42;
const THICKNESS = 1.2;
const GAP = 0.22;
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
  now,
}: {
  buff: BuffTimer;
  from: number;
  to: number;
  now: number;
}) {
  const left =
    buff.ends_at === null ? null : Math.max(0, (buff.ends_at - now) / 1000);
  const share =
    left === null ? 0 : Math.min(1, left / Math.max(buff.total, 0.001));
  const colour = left === null ? DOWN : share < WARN_SHARE ? WARN : INK;
  const middle = (from + to) / 2;
  const [x, y] = point(middle, RADIUS + LABEL_OFFSET);
  const anchor =
    Math.abs(Math.sin(middle)) < 0.3
      ? "middle"
      : Math.sin(middle) > 0
        ? "start"
        : "end";

  return (
    <g>
      <path
        d={arc(from, to)}
        stroke={left === null ? DOWN : INK}
        strokeOpacity={left === null ? 0.5 : 0.18}
        strokeWidth={THICKNESS}
        fill="none"
      />
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
        fillOpacity={0.8}
        textAnchor={anchor}
        dominantBaseline="middle"
        fontSize={7}
        letterSpacing={0.4}
        style={{
          textTransform: "uppercase",
          filter: "drop-shadow(0 0 0.6px rgb(0 0 0 / 0.9))",
        }}
      >
        {buff.label}{" "}
        <tspan fontWeight={600} fillOpacity={1}>
          {left === null ? "—" : Math.ceil(left)}
        </tspan>
      </text>
    </g>
  );
}

export function BuffRing({
  buffs,
  opacity,
}: {
  buffs: BuffTimer[];
  opacity: number;
}) {
  const now = useNow();
  const span = (2 * Math.PI) / Math.max(buffs.length, 1);

  return (
    <svg
      viewBox={`0 0 ${SIZE} ${SIZE}`}
      className="size-full"
      style={{ opacity: opacity / 100 }}
    >
      {buffs.map((buff, index) => {
        const from = index * span + GAP / 2;
        return (
          <Buff
            key={buff.id}
            buff={buff}
            from={from}
            to={from + span - GAP}
            now={now}
          />
        );
      })}
    </svg>
  );
}
