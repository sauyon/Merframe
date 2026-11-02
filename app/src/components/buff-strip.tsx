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
// An ability keeping up several buffs at once, like Tribute, has a marker under its number
// for each buff: the game's icon for it when it has one, else a pip. When the ability draws
// its own widget for them, the number and markers follow it: the number is the buff the player
// has switched to, its marker first and larger, the rest in the order switching goes through
// them, so cycling in game cycles them. Otherwise the number is the buff that runs out first.
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
/** The pip of the buff the number shows, when the ability's own widget picks it. */
const LEAD_PIP_RADIUS = 1.6;
const PIP_GAP = 3.6;
/** Icons standing in for pips, when the game has one per buff. */
const MARKER_SIZE = 5;
const LEAD_MARKER_SIZE = 7;
const MARKER_GAP = 1;
/** Extra room between the ability slots and the other buffs, in pitches. */
const GROUP_GAP = 0.5;
/** The number warms once this share of the buff is left, */
const WARN_SHARE = 0.25;
/** or, for an ability's buffs, once this many seconds are: long enough to recast. A buff's full
 * length is only known from seeing it applied, so this also covers ones first seen part way
 * through. Other buffs, like arcane procs, can be shorter than this. */
const WARN_SECONDS = 10;
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
  if (left === null || left <= 0) {
    return DOWN;
  }
  const share = Math.min(1, left / Math.max(buff.total, 0.001));
  const late = buff.slot !== null && left <= WARN_SECONDS;
  return share < WARN_SHARE || late ? WARN : INK;
}

/** SVG filters painting an icon's shape one flat colour, keyed by colour. */
const TINTS = { [INK]: "tint-ink", [WARN]: "tint-warn", [DOWN]: "tint-down" };

function Tints() {
  return (
    <defs>
      {Object.entries(TINTS).map(([colour, id]) => (
        <filter key={id} id={id}>
          <feFlood floodColor={colour} />
          <feComposite in2="SourceAlpha" operator="in" />
          <feDropShadow dx={0} dy={0} stdDeviation={0.5} floodColor="black" />
        </filter>
      ))}
    </defs>
  );
}

/** One ability's buffs, side by side under its number, each coloured by its own share left. */
function Markers({
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
  if (!buffs.every((buff) => buff.icon)) {
    return <Pips buffs={buffs} x={x} y={y} now={now} />;
  }
  const size = (buff: BuffTimer) =>
    buff.rank === 0 ? LEAD_MARKER_SIZE : MARKER_SIZE;
  const width =
    buffs.reduce((sum, buff) => sum + size(buff), 0) +
    (buffs.length - 1) * MARKER_GAP;
  let left = x - width / 2;
  return (
    <g>
      {buffs.map((buff) => {
        const at = left;
        left += size(buff) + MARKER_GAP;
        return (
          <Marker
            key={buff.id}
            name={buff.icon ?? ""}
            x={at}
            y={y - size(buff) / 2 + MARKER_SIZE / 2}
            size={size(buff)}
            tint={TINTS[colourOf(buff, now)]}
          />
        );
      })}
    </g>
  );
}

function Marker({
  name,
  x,
  y,
  size,
  tint,
}: {
  name: string;
  x: number;
  y: number;
  size: number;
  tint: string;
}) {
  const src = useItemImageSrc(name);
  if (!src) {
    return null;
  }
  return (
    <image
      href={src}
      x={x}
      y={y}
      width={size}
      height={size}
      filter={`url(#${tint})`}
      opacity={0.92}
    />
  );
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
          cy={y + PIP_GAP / 2}
          r={buff.rank === 0 ? LEAD_PIP_RADIUS : PIP_RADIUS}
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
      {left === null || left <= 0 ? "—" : Math.ceil(left)}
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

/** Of several buffs from one ability, the one the player has switched to in its widget, else
 * the running one that ends first. */
function pick(buffs: BuffTimer[]) {
  const lead = buffs.find((buff) => buff.rank === 0);
  if (lead) {
    return lead;
  }
  const end = (buff: BuffTimer) => buff.ends_at ?? Number.POSITIVE_INFINITY;
  return buffs.reduce((best, buff) => (end(buff) < end(best) ? buff : best));
}

/** One ability's buffs in the order its widget cycles through them, when it has one. */
function ordered(buffs: BuffTimer[]) {
  const rank = (buff: BuffTimer) => buff.rank ?? Number.POSITIVE_INFINITY;
  return [...buffs].sort((a, b) => rank(a) - rank(b));
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
    buffs: ordered(buffs.filter((buff) => buff.slot === index + 1)),
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
      <Tints />
      {slots.map(
        ({ x, buffs }) =>
          buffs.length > 0 && (
            <g key={x}>
              <Buff buff={pick(buffs)} x={x} now={now} fontSize={fontSize} />
              {buffs.length > 1 && (
                <Markers buffs={buffs} x={x} y={TOP + fontSize} now={now} />
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
