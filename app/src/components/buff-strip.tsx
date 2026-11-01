import { useEffect, useState } from "react";
import type { BuffTimer, TextSize } from "@/types";

// Countdown seconds just under the game's ability dots, which sit at the same place below
// the screen centre whatever the weapon, unlike its reticles. Abilities 1 to 4 keep fixed
// slots, each number right under its dot; other buffs flank them. Styled like the HUD:
// off-white numbers, colour only as a recast hint.
//
// The window is sized to the screen height alone, so 1 unit is 1 px at 1080p whatever the
// text size; that only scales the numbers. The dots are measured in game at 2160p: 15 units
// apart, centred 77.5 units below the middle.
const SIZE = 440;
const CENTRE = SIZE / 2;
/** Ability slots in the strip. */
const SLOTS = 4;
/** The dots' spacing; each ability's number sits right under its dot. */
const PITCH = 15;
/** Where the numbers' tops sit, just below the dots. */
const TOP = CENTRE + 83;
/** Seconds at small text, scaled like the app's root size. */
const FONT_SIZE = 11;
const TEXT_SCALE: Record<TextSize, number> = {
  small: 1,
  medium: 1.125,
  large: 1.25,
};
/** Extra room between the ability slots and the other buffs, in pitches. */
const GROUP_GAP = 0.5;
/** The number warms once this share of the buff is left. */
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

/** The middle of the slot at `position`, counted in pitches from the strip's middle. */
function slotX(position: number) {
  return CENTRE + position * PITCH;
}

function Buff({
  buff,
  x,
  now,
  fontSize,
}: {
  buff: BuffTimer;
  x: number;
  now: number;
  fontSize: number;
}) {
  const left =
    buff.ends_at === null ? null : Math.max(0, (buff.ends_at - now) / 1000);
  const share =
    left === null ? 0 : Math.min(1, left / Math.max(buff.total, 0.001));
  const colour = left === null ? DOWN : share < WARN_SHARE ? WARN : INK;

  return (
    <text
      x={x}
      y={TOP + fontSize / 2}
      fill={colour}
      fillOpacity={0.92}
      fontWeight={600}
      textAnchor="middle"
      dominantBaseline="middle"
      fontSize={fontSize}
      style={{ filter: "drop-shadow(0 0 1px rgb(0 0 0 / 0.9))" }}
    >
      {left === null ? "—" : Math.ceil(left)}
    </text>
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
  textSize,
}: {
  buffs: BuffTimer[];
  opacity: number;
  textSize: TextSize;
}) {
  const now = useNow();
  const fontSize = FONT_SIZE * TEXT_SCALE[textSize];
  const slots = Array.from({ length: SLOTS }, (_, index) => ({
    x: slotX(index - (SLOTS - 1) / 2),
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
      {slots.map(
        ({ x, buffs }) =>
          buffs.length > 0 && (
            <Buff
              key={x}
              buff={pick(buffs)}
              x={x}
              now={now}
              fontSize={fontSize}
            />
          ),
      )}
      {others.map((buff, index) => (
        <Buff
          key={buff.id}
          buff={buff}
          x={slotX(flank(index))}
          now={now}
          fontSize={fontSize}
        />
      ))}
    </svg>
  );
}
