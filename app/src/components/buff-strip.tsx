import { useEffect, useState } from "react";
import type { BuffTimer } from "@/types";

// A strip of thin countdown bars just under the game's ability dots, which sit at the same
// place below the screen centre whatever the weapon, unlike its reticles. Abilities 1 to 4
// keep fixed slots, left to right like the dots; other buffs flank them. Styled like the HUD:
// off-white strokes, small seconds left under each bar, colour only as a recast hint.
//
// Units are 1/440 of the window, which is sized to the screen height, so 1 unit is 3 px at
// 2160p. The dots are measured there: 10 units apart, centred 52 units below the middle.
const SIZE = 440;
const CENTRE = SIZE / 2;
/** Ability slots in the strip. */
const SLOTS = 4;
/** The dots' spacing; each ability's bar sits right under its dot. */
const PITCH = 10;
/** Where the bars run, just below the dots. */
const BAR_Y = CENTRE + 57;
const BAR_WIDTH = 7.5;
const THICKNESS = 1.2;
/** Seconds sit under their bar. */
const LABEL_Y = BAR_Y + 5.5;
/** Extra room between the ability slots and the other buffs, in pitches. */
const GROUP_GAP = 0.5;
/** The bar warms once this share of the buff is left. */
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

/** The left edge of the bar at `position`, counted in pitches from the strip's middle. */
function barX(position: number) {
  return CENTRE + position * PITCH - BAR_WIDTH / 2;
}

function Track({ x }: { x: number }) {
  return (
    <line
      x1={x}
      x2={x + BAR_WIDTH}
      y1={BAR_Y}
      y2={BAR_Y}
      stroke={INK}
      strokeOpacity={0.14}
      strokeWidth={THICKNESS}
    />
  );
}

function Buff({ buff, x, now }: { buff: BuffTimer; x: number; now: number }) {
  const left =
    buff.ends_at === null ? null : Math.max(0, (buff.ends_at - now) / 1000);
  const share =
    left === null ? 0 : Math.min(1, left / Math.max(buff.total, 0.001));
  const colour = left === null ? DOWN : share < WARN_SHARE ? WARN : INK;

  return (
    <g>
      <line
        x1={x}
        x2={x + BAR_WIDTH * (left === null ? 1 : share)}
        y1={BAR_Y}
        y2={BAR_Y}
        stroke={colour}
        strokeOpacity={left === null ? 0.5 : 0.85}
        strokeWidth={THICKNESS}
      />
      <text
        x={x + BAR_WIDTH / 2}
        y={LABEL_Y}
        fill={colour}
        fillOpacity={0.9}
        fontWeight={600}
        textAnchor="middle"
        dominantBaseline="middle"
        fontSize={5.5}
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

/** Where the `index`th other buff goes: alternately right then left of the ability slots. */
function flank(index: number) {
  const out = SLOTS / 2 + 0.5 + GROUP_GAP + Math.floor(index / 2);
  return index % 2 === 0 ? out : -out;
}

export function BuffStrip({
  buffs,
  opacity,
}: {
  buffs: BuffTimer[];
  opacity: number;
}) {
  const now = useNow();
  const slots = Array.from({ length: SLOTS }, (_, index) => ({
    x: barX(index - (SLOTS - 1) / 2),
    buffs: buffs.filter((buff) => buff.slot === index + 1),
  }));
  const others = buffs.filter(
    (buff) => buff.slot === null || buff.slot > SLOTS,
  );

  return (
    <svg
      viewBox={`0 0 ${SIZE} ${SIZE}`}
      className="size-full"
      style={{ opacity: opacity / 100 }}
    >
      {slots.map(({ x, buffs }) => (
        <g key={x}>
          <Track x={x} />
          {buffs.length > 0 && <Buff buff={pick(buffs)} x={x} now={now} />}
        </g>
      ))}
      {others.map((buff, index) => {
        const x = barX(flank(index));
        return (
          <g key={buff.id}>
            <Track x={x} />
            <Buff buff={buff} x={x} now={now} />
          </g>
        );
      })}
    </svg>
  );
}
