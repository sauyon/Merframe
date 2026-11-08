import { useEffect, useState } from "react";
import { useItemImageSrc } from "@/components/item-image";
import type {
  BuffLayout,
  BuffStyle,
  BuffTimer,
  TextSize,
  Vitals,
} from "@/types";

// Countdown seconds over the game's ability dots, which sit at the same place below the
// screen centre whatever the weapon, unlike its reticles. Abilities 1 to 4 keep fixed slots,
// each number covering its dot; other buffs flank them, each under the game's icon for it
// standing in for a dot. Styled like the HUD: off-white numbers and icons, colour only
// as a recast hint.
//
// The window is sized to the screen height alone, so 1 unit is 1 px at 1080p whatever the
// text size; that only scales the numbers. The strip is drawn as the dots were measured at
// HUD scale 1 at 2160p: 12 units apart, centred 63.25 units below the middle. It then moves
// to where the HUD placed the dots' clip and scales with the HUD, both read from the game, so
// it follows the HUD scale and borders. The HUD scales about the screen's middle, so the
// dots' distance from it grows with the scale as well as their size.
//
// An ability keeping up several buffs at once, like Tribute, has a marker under its number
// for each buff: the game's icon for it when it has one, else a pip. When the ability draws
// its own widget for them, the number and markers follow it: the number is the buff the player
// has switched to, its marker first and larger, the rest in the order switching goes through
// them, so cycling in game cycles them. Otherwise the number is the buff that runs out first.
// When the number is not the running buff that runs out first, that one's seconds sit smaller
// under the markers in a colour of their own, so a lapsing buff is never hidden by the cycle.
//
// Just above the dots, as wide as the ability slots, sit the player's health and energy as
// the HUD shows them, filling from the right as its own bars do: shields covering health, so
// it shows as they drain, overshields and overguard as lines over those, and energy on a bar
// under them.
const SIZE = 440;
const CENTRE = SIZE / 2;
/** Ability slots in the strip. */
const SLOTS = 4;
/** The dots' spacing; each ability's number sits over its dot. */
const PITCH = 12;
/** The dots' centre line. */
const DOT_Y = CENTRE + 63.25;
/** The dots' spacing in the HUD's stage units at HUD scale 1. */
const STAGE_PITCH = 10;
/** How far the dots' centre sits below their clip's origin, likewise. */
const STAGE_DROP = 0.7;
/** Strip units are px at this screen height. */
const UNIT_HEIGHT = 1080;
/** Icons of the other buffs, centred on the dots' line, and their spacing. */
const ICON_SIZE = 18;
const ICON_PITCH = ICON_SIZE + 2;
/** Where the other buffs' numbers' tops sit, tucked just under their icons. */
const TOP = DOT_Y + ICON_SIZE / 2 - 1.5;
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
/** The number warms once this share of the buff is left, or, for an ability's buffs, once the
 * settings' seconds are: long enough to recast. A buff's full length is only known from seeing
 * it applied, so the seconds also cover ones first seen part way through. */
const WARN_SHARE = 0.25;
const INK = "rgb(235 238 240)";
const WARN = "rgb(240 190 110)";
const DOWN = "rgb(235 110 95)";
/** The running buff that runs out first, when the number shows another. */
const LOWEST = "rgb(130 190 245)";
const LOWEST_SCALE = 0.75;
/** A disc hiding the game's ability dot, which each ability's number stands on in its place:
 * solid whatever the strip's opacity, as wide as the dots' spacing allows. Dark while the buff
 * has time; once it is nearly out or gone the disc takes the warning colour and the number
 * goes dark on it. */
const BACKING = "rgb(12 14 16)";
const BACKING_RADIUS = PITCH / 2 - 0.5;
/** A digit's width as a share of its size: an ability's number shrinks to fit its disc. */
const DIGIT_WIDTH = 0.56;
/** Digits' height above their baseline, as a share of their size: half of it drops their
 * middle onto a centre line. */
const DIGIT_HEIGHT = 0.72;
/** The health and energy bars, above the dots' discs. */
const BAR_WIDTH = SLOTS * PITCH;
const BAR_HEIGHT = 2;
const BAR_GAP = 1.5;
const ENERGY_Y = DOT_Y - BACKING_RADIUS - 4 - BAR_HEIGHT;
const HEALTH_Y = ENERGY_Y - BAR_GAP - BAR_HEIGHT;
const OVERGUARD_HEIGHT = 0.8;
/** A bar's colour while the HUD's own can't be read. */
const UNREAD = INK;
const EMPTY = "rgb(12 14 16 / 0.6)";

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

/** Moves the strip, drawn around the measured dots, onto the dots as the HUD placed them. */
function hudTransform(layout: BuffLayout | null) {
  if (!layout) {
    return "";
  }
  const unit = (UNIT_HEIGHT / layout.stage_height) * layout.hud_scale;
  const scale = (unit * STAGE_PITCH) / PITCH;
  const x = CENTRE + (layout.x - layout.stage_width / 2) * unit;
  const y = CENTRE + (layout.y - layout.stage_height / 2 + STAGE_DROP) * unit;
  return `translate(${x} ${y}) scale(${scale}) translate(${-CENTRE} ${-DOT_Y})`;
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
function colourOf(buff: BuffTimer, now: number, warn: number) {
  const left = remaining(buff, now);
  if (left === null || left <= 0) {
    return DOWN;
  }
  const share = Math.min(1, left / Math.max(buff.total, 0.001));
  const late = buff.slot !== null && left <= warn;
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
  warn,
}: {
  buffs: BuffTimer[];
  x: number;
  y: number;
  now: number;
  warn: number;
}) {
  if (!buffs.every((buff) => buff.icon)) {
    return <Pips buffs={buffs} x={x} y={y} now={now} warn={warn} />;
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
            tint={TINTS[colourOf(buff, now, warn)]}
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
  warn,
}: {
  buffs: BuffTimer[];
  x: number;
  y: number;
  now: number;
  warn: number;
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
          fill={colourOf(buff, now, warn)}
          fillOpacity={0.92}
        />
      ))}
    </g>
  );
}

/** The disc's colour under an ability's number: dark, or the number's warning colour. */
function discOf(buff: BuffTimer, now: number, warn: number) {
  const state = colourOf(buff, now, warn);
  return state === INK ? BACKING : state;
}

function Buff({
  buff,
  x,
  y,
  now,
  warn,
  fontSize,
  width,
  onDisc,
}: {
  buff: BuffTimer;
  x: number;
  /** The number's centre line. */
  y: number;
  now: number;
  warn: number;
  fontSize: number;
  /** The most room the number may take across, if limited. */
  width?: number;
  /** Stands on a disc of its own colour, so the number is drawn dark on it. */
  onDisc?: boolean;
}) {
  const left = remaining(buff, now);
  const state = colourOf(buff, now, warn);
  const colour = onDisc && state !== INK ? BACKING : state;
  const text = left === null || left <= 0 ? "—" : String(Math.ceil(left));
  const size =
    width === undefined
      ? fontSize
      : Math.min(fontSize, width / (text.length * DIGIT_WIDTH));

  return (
    <text
      x={x}
      y={y + (size * DIGIT_HEIGHT) / 2}
      fill={colour}
      fillOpacity={0.92}
      fontWeight={600}
      textAnchor="middle"
      dominantBaseline="alphabetic"
      fontSize={size}
      style={{ filter: "drop-shadow(0 0 1px rgb(0 0 0 / 0.9))" }}
    >
      {text}
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

/** Of several buffs from one ability, the running one that ends first. */
function lowest(buffs: BuffTimer[], now: number) {
  const running = buffs.filter((buff) => (remaining(buff, now) ?? 0) > 0);
  const end = (buff: BuffTimer) => buff.ends_at ?? Number.POSITIVE_INFINITY;
  return running.length === 0
    ? null
    : running.reduce((best, buff) => (end(buff) < end(best) ? buff : best));
}

/** The seconds of the buff that runs out first, smaller, under one ability's markers. */
function Lowest({
  buff,
  x,
  y,
  now,
  fontSize,
}: {
  buff: BuffTimer;
  x: number;
  y: number;
  now: number;
  fontSize: number;
}) {
  const left = remaining(buff, now) ?? 0;
  return (
    <text
      x={x}
      y={y + fontSize / 2}
      fill={LOWEST}
      fillOpacity={0.92}
      fontWeight={600}
      textAnchor="middle"
      dominantBaseline="middle"
      fontSize={fontSize}
      style={{ filter: "drop-shadow(0 0 1px rgb(0 0 0 / 0.9))" }}
    >
      {Math.ceil(left)}
    </text>
  );
}

/** One ability's buffs in the order its widget cycles through them, when it has one. */
function ordered(buffs: BuffTimer[]) {
  const rank = (buff: BuffTimer) => buff.rank ?? Number.POSITIVE_INFINITY;
  return [...buffs].sort((a, b) => rank(a) - rank(b));
}

/** A bar's fill, as a share of its width. */
function share(value: number, of: number) {
  return of > 0 ? Math.min(1, Math.max(0, value / of)) : 0;
}

/** A colour the HUD draws a bar in, `0xRRGGBB`, as CSS. */
function colour(rgb: number | null) {
  return rgb === null ? UNREAD : `#${rgb.toString(16).padStart(6, "0")}`;
}

/** Shields covering health, so the health shows as they drain, overshields and overguard as
 * lines over them, and energy under them, each against its own maximum. They fill from the
 * right, as the HUD's own bars do. */
function VitalBars({ vitals }: { vitals: Vitals }) {
  const right = CENTRE + BAR_WIDTH / 2;
  /** A fill reaching `start` to `end` of the bar's width in from its right end. */
  const fill = (
    key: string,
    y: number,
    height: number,
    start: number,
    end: number,
    colour: string,
  ) =>
    end > start && (
      <rect
        key={key}
        x={right - end * BAR_WIDTH}
        y={y}
        width={(end - start) * BAR_WIDTH}
        height={height}
        fill={colour}
      />
    );
  const health = share(vitals.health, vitals.max_health);
  const shield = share(vitals.shield, vitals.max_shield);
  const overshield = share(
    vitals.shield - vitals.max_shield,
    vitals.max_overshield,
  );
  const overguard = share(
    vitals.overguard,
    Math.max(vitals.max_overguard, vitals.overguard),
  );
  const energy = share(vitals.energy, vitals.max_energy);
  const lineY = HEALTH_Y - OVERGUARD_HEIGHT;
  return (
    <g style={{ filter: "drop-shadow(0 0 1px rgb(0 0 0 / 0.9))" }}>
      {fill("empty", HEALTH_Y, BAR_HEIGHT, 0, 1, EMPTY)}
      {fill(
        "health",
        HEALTH_Y,
        BAR_HEIGHT,
        0,
        health,
        colour(vitals.health_colour),
      )}
      {fill(
        "shield",
        HEALTH_Y,
        BAR_HEIGHT,
        0,
        shield,
        colour(vitals.shield_colour),
      )}
      {fill(
        "overshield",
        lineY,
        OVERGUARD_HEIGHT,
        0,
        overshield,
        colour(vitals.overshield_colour),
      )}
      {fill(
        "overguard",
        lineY - (overshield > 0 ? OVERGUARD_HEIGHT : 0),
        OVERGUARD_HEIGHT,
        0,
        overguard,
        colour(vitals.overguard_colour),
      )}
      {fill("energy-empty", ENERGY_Y, BAR_HEIGHT, 0, 1, EMPTY)}
      {fill(
        "energy",
        ENERGY_Y,
        BAR_HEIGHT,
        0,
        energy,
        colour(vitals.energy_colour),
      )}
    </g>
  );
}

/** Where the `index`th other buff goes: alternately right then left of the ability slots. */
function flank(index: number) {
  const out =
    (SLOTS / 2 + GROUP_GAP) * PITCH +
    (0.5 + Math.floor(index / 2)) * ICON_PITCH;
  return CENTRE + (index % 2 === 0 ? out : -out);
}

export function BuffStrip({
  buffs,
  opacity,
  textSize,
  style,
  layout,
  vitals,
}: {
  buffs: BuffTimer[];
  opacity: number;
  textSize: TextSize;
  style: BuffStyle;
  layout: BuffLayout | null;
  vitals: Vitals | null;
}) {
  const now = useNow();
  const warn = style.warn_seconds;
  const scale = style.scale / 100;
  const fontSize = FONT_SIZE * TEXT_SCALE[textSize] * scale;
  const covers = style.cover_dots;
  /** An ability's number: on its dot, or just under it. */
  const numberY = covers ? DOT_Y : DOT_Y + BACKING_RADIUS + fontSize / 2;
  const slots = Array.from({ length: SLOTS }, (_, index) => ({
    x: slotX(index - (SLOTS - 1) / 2),
    buffs: ordered(buffs.filter((buff) => buff.slot === index + 1)),
  }));
  const others = buffs.filter(
    (buff) => buff.slot === null || buff.slot > SLOTS,
  );

  return (
    <svg viewBox={`0 0 ${SIZE} ${SIZE}`} className="size-full">
      <Tints />
      <g transform={`translate(0 ${style.offset}) ${hudTransform(layout)}`}>
        {slots.map(
          ({ x, buffs }) =>
            covers &&
            buffs.length > 0 && (
              <circle
                key={x}
                cx={x}
                cy={DOT_Y}
                r={BACKING_RADIUS}
                fill={discOf(pick(buffs), now, warn)}
              />
            ),
        )}
        <g opacity={opacity / 100}>
          {vitals && <VitalBars vitals={vitals} />}
          {slots.map(({ x, buffs }) => {
            if (buffs.length === 0) {
              return null;
            }
            const shown = pick(buffs);
            const first = lowest(buffs, now);
            return (
              <g key={x}>
                <Buff
                  buff={shown}
                  x={x}
                  y={numberY}
                  now={now}
                  warn={warn}
                  fontSize={fontSize}
                  width={BACKING_RADIUS * 2 - 1}
                  onDisc={covers}
                />
                {buffs.length > 1 && (
                  <Markers
                    buffs={buffs}
                    x={x}
                    y={numberY + fontSize / 2}
                    now={now}
                    warn={warn}
                  />
                )}
                {first && first.id !== shown.id && (
                  <Lowest
                    buff={first}
                    x={x}
                    y={numberY + fontSize / 2 + LEAD_MARKER_SIZE + MARKER_GAP}
                    now={now}
                    fontSize={fontSize * LOWEST_SCALE}
                  />
                )}
              </g>
            );
          })}
          <g
            transform={`translate(${CENTRE} ${DOT_Y}) scale(${scale}) translate(${-CENTRE} ${-DOT_Y})`}
          >
            {others.map((buff, index) => {
              const x = flank(index);
              return (
                <g key={buff.id}>
                  {buff.icon && <Icon name={buff.icon} x={x} />}
                  <Buff
                    buff={buff}
                    x={x}
                    y={TOP + fontSize / scale / 2}
                    now={now}
                    warn={warn}
                    fontSize={fontSize / scale}
                  />
                </g>
              );
            })}
          </g>
        </g>
      </g>
    </svg>
  );
}
