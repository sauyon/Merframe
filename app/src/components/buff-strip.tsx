import { useEffect, useState } from "react";
import { useItemImageSrc } from "@/components/item-image";
import type { BuffTimer, TextSize } from "@/types";

// Countdown seconds just under the game's ability dots, which sit at the same place below
// the screen centre whatever the weapon, unlike its reticles. Abilities 1 to 4 keep fixed
// slots, each number right under its dot; other buffs flank them, each under the game's icon
// for it standing in for a dot. Styled like the HUD: off-white numbers and icons, colour only
// as a recast hint.
//
// The window is sized to the screen height alone, so 1 unit is 1 px at 1080p whatever the
// text size; that only scales the numbers. The dots are measured in game at 2160p: 15 units
// apart, centred 77.5 units below the middle.
//
// An ability keeping up several buffs at once, like Tribute, shows the one that runs out
// first, with a pip under it for each buff.
const SIZE = 440;
const CENTRE = SIZE / 2;
/** Ability slots in the strip. */
const SLOTS = 4;
/** The dots' spacing; each ability's number sits right under its dot. */
const PITCH = 15;
/** The dots' centre line. */
const DOT_Y = CENTRE + 77.5;
/** Where the numbers' tops sit, just below the dots. */
const TOP = CENTRE + 83;
/** Icons of the other buffs, centred on the dots' line. */
const ICON_SIZE = 14;
/** Seconds at small text, scaled like the app's root size. */
const FONT_SIZE = 11;
const TEXT_SCALE: Record<TextSize, number> = {
  small: 1,
  medium: 1.125,
  large: 1.25,
};
/** Pips under a slot's number, one per buff the ability keeps up. */
const PIP_RADIUS = 1.1;
const PIP_GAP = 3.6;
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

/** How much of the buff is left, or `null` once it has dropped off. */
function remaining(buff: BuffTimer, now: number) {
  return buff.ends_at === null
    ? null
    : Math.max(0, (buff.ends_at - now) / 1000);
}

/** The number's colour: off-white, warmer when nearly out, red once gone. */
function colourOf(buff: BuffTimer, now: number) {
  const left = remaining(buff, now);
  const share =
    left === null ? 0 : Math.min(1, left / Math.max(buff.total, 0.001));
  return left === null ? DOWN : share < WARN_SHARE ? WARN : INK;
}

/** A dot per buff of one ability, each coloured by its own share left. */
function Pips({
  buffs,
  x,
  y,
  now,
}: {
  buffs: BuffTimer[];
  x: number;
  y: number;
  now: number;
}) {
  const start = x - ((buffs.length - 1) * PIP_GAP) / 2;
  return (
    <g style={{ filter: "drop-shadow(0 0 1px rgb(0 0 0 / 0.9))" }}>
      {buffs.map((buff, index) => (
        <circle
          key={buff.id}
          cx={start + index * PIP_GAP}
          cy={y}
          r={PIP_RADIUS}
          fill={colourOf(buff, now)}
          fillOpacity={0.92}
        />
      ))}
    </g>
  );
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
  const left = remaining(buff, now);
  const colour = colourOf(buff, now);

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

/** The game's icon for a buff, tinted off-white like the HUD's own buff icons. */
function Icon({ name, x }: { name: string; x: number }) {
  const src = useItemImageSrc(name);
  if (!src) {
    return null;
  }
  return (
    <image
      href={src}
      x={x - ICON_SIZE / 2}
      y={DOT_Y - ICON_SIZE / 2}
      width={ICON_SIZE}
      height={ICON_SIZE}
      style={{
        filter:
          "brightness(0) invert(0.93) drop-shadow(0 0 1px rgb(0 0 0 / 0.9))",
      }}
    />
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
            <g key={x}>
              <Buff buff={pick(buffs)} x={x} now={now} fontSize={fontSize} />
              {buffs.length > 1 && (
                <Pips
                  buffs={buffs}
                  x={x}
                  y={TOP + fontSize + PIP_GAP / 2}
                  now={now}
                />
              )}
            </g>
          ),
      )}
      {others.map((buff, index) => {
        const x = slotX(flank(index));
        return (
          <g key={buff.id}>
            {buff.icon && <Icon name={buff.icon} x={x} />}
            <Buff buff={buff} x={x} now={now} fontSize={fontSize} />
          </g>
        );
      })}
    </svg>
  );
}
