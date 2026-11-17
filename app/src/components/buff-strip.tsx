import "@fontsource/roboto/latin-700.css";
import { useEffect, useState } from "react";
import { useItemImageSrc } from "@/components/item-image";
import type {
  BuffLayout,
  BuffStyle,
  BuffTimer,
  TextSize,
  Vitals,
} from "@/types";

// Ability buffs as thin rings round the game's ability dots, which sit at the same place below
// the screen centre whatever the weapon, unlike its reticles. Each ring drains as its buff runs
// out; once the settings' seconds are left it thickens and turns amber, with the seconds under
// the dot, and when the buff ends it pulses red. An empty slot draws nothing. The game turns an
// ability's dot cyan while it is active, so the rings stay white to read apart from it.
//
// Under the rings sit the player's vitals as two short meters, shields (health once they break,
// thicker so it shows on warm scenes) on the left and energy on the right, each filling from its
// outer end inward. They carry no warning colours, so amber and red only ever mean a timer, and
// they fade back while both are nearly full. Of the other buffs, only the one about to run out
// shows, its icon and seconds well right of the dots so it doesn't read as a fifth ability.
// No number shows away from its ability's dot without saying what it counts: where there's no
// icon, its name follows the seconds, small and cut short, like the operator's Focus cooldown.
//
// Everything stays below the crosshair, as the game keeps its own fixed reticle parts: clear of
// the reticle's spread, which the app can't read, and of world markers, which sit above it. It
// is drawn soft, with a slight blur and glow, to sit with the HUD's filtered textures.
//
// The window is sized to the screen height alone, so 1 unit is 1 px at 1080p whatever the
// text size; that only scales the numbers. The strip is drawn as the dots were measured at
// HUD scale 1 at 2160p: 12 units apart, centred 63.25 units below the middle. It then moves
// to where the HUD placed the dots' clip and scales with the HUD, both read from the game, so
// it follows the HUD scale and borders. The HUD scales about the screen's middle, so the
// dots' distance from it grows with the scale as well as their size.
//
// An ability keeping up several buffs at once, like Tribute, gets a tray on its own side of the
// dots, out past the meters: a row per running buff, its icon and seconds, each coloured by its
// own time left so a lapsing buff is never hidden. The rows keep their places, in the order the
// ability's widget cycles through them; the buff the player has switched to is marked larger,
// lit and ticked on the side facing the dots, and is the one the ring shows, so cycling in game
// moves the mark, not the rows. Without such a widget the mark is the buff that ends first. A
// tray on the right puts the urgent other buff under it.
const SIZE = 440;
const CENTRE = SIZE / 2;
/** Ability slots in the strip. */
const SLOTS = 4;
/** The dots' spacing; each ability's ring sits round its dot. */
const PITCH = 12;
/** The dots' centre line. */
const DOT_Y = CENTRE + 63.25;
/** The dots' spacing in the HUD's stage units at HUD scale 1. */
const STAGE_PITCH = 10;
/** How far the dots' centre sits below their clip's origin, likewise. */
const STAGE_DROP = 0.7;
/** Strip units are px at this screen height. */
const UNIT_HEIGHT = 1080;
/** The rings: clear of the game's cyan dot and its glow, of each other, and of the reticle's
 * widest spread above. */
const RING_RADIUS = 4.2;
const RING_WIDTH = 1.25;
const RING_WARN_WIDTH = 2;
/** The red pulse of an ended buff, per second: it shows by motion where red is lost. */
const PULSE_RATE = 4;
/** The vitals meters: from `METER_GAP` to `METER_END` either side of the middle, `METER_DROP`
 * below the dots, curving up `METER_CURVE` at their ends. */
const METER_GAP = 6;
const METER_END = 32;
const METER_DROP = 11;
const METER_CURVE = 1.6;
const METER_WIDTH = 2.2;
const HEALTH_WIDTH = 3;
const OVER_WIDTH = 0.8;
/** The meters' opacity while shields and energy are both above `FULL`. */
const FULL = 0.9;
const FULL_OPACITY = 0.45;
/** The urgent other buff, right of the last dot. */
const OTHER_X = CENTRE + 1.5 * PITCH + 32;
const ICON_SIZE = 9;
/** The urgent buff's seconds, as a share of the ability seconds' size. */
const OTHER_TEXT = 0.85;
const OTHER_OPACITY = 0.85;
/** Seconds at small text, scaled like the app's root size. */
const FONT_SIZE = 13;
const TEXT_SCALE: Record<TextSize, number> = {
  small: 1,
  medium: 1.125,
  large: 1.25,
};
/** The tray of an ability's several buffs: its icons' column this far either side of the
 * middle, clear of the meters' ends; rows from just above the dots' line down. */
const TRAY_OUT = 56;
/** A second tray on the same side, this much further out. */
const TRAY_STEP = 46;
/** The urgent other buff's extra room under a tray, so it doesn't read as one more row. */
const URGENT_BREAK = 6;
const TRAY_TOP = DOT_Y - 2;
const TRAY_ROW = 12;
const TRAY_ICON = 10;
const TRAY_LEAD_ICON = 13;
/** Unmarked rows, set back and smaller. */
const TRAY_REST_OPACITY = 0.75;
const TRAY_REST_TEXT = 0.85;
/** A dot for a buff the game has no icon for. */
const TRAY_DOT = 2.2;
/** The mark's tick, on the side facing the dots. */
const TICK_HEIGHT = 7;
const TICK_WIDTH = 1.5;
/** A name beside seconds, for a buff with no icon: smaller than the seconds and cut short. */
const NOTE_TEXT = 0.7;
const NOTE_CHARS = 12;
/** Room between the rows under the rings. */
const ROW_SPACE = 2;
/** A buff warms once this share of it is left, or, for an ability's buffs, once the settings'
 * seconds are: long enough to recast. A buff's full length is only known from seeing it
 * applied, so the seconds also cover ones first seen part way through. */
const WARN_SHARE = 0.25;
/** The HUD's typeface for its numbers. */
const FONT = "Roboto, sans-serif";
/** A slightly dimmed white, like the game's own dots. */
const INK = "rgb(238 243 245)";
const WARN = "rgb(255 198 64)";
const DOWN = "rgb(255 77 94)";
/** Shields and health take the HUD's own colours; these stand in until those are read. */
const SHIELD = "rgb(156 196 240)";
const HEALTH = "rgb(255 140 122)";
const ENERGY = INK;
/** A colour the HUD draws a line in, for the overshield and overguard lines, while unread. */
const UNREAD = INK;
/** Digits' height above their baseline, as a share of their size: half of it drops their
 * middle onto a centre line. */
const DIGIT_HEIGHT = 0.72;
/** A dark outline under numbers, so they hold on bright scenes. */
const TEXT_OUTLINE = {
  stroke: "black",
  strokeWidth: 2,
  strokeOpacity: 0.55,
  paintOrder: "stroke",
} as const;

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

/** A buff's colour: white, amber when nearly out, red once gone. */
function colourOf(buff: BuffTimer, now: number, warn: number) {
  const left = remaining(buff, now);
  if (left === null || left <= 0) {
    return DOWN;
  }
  const share = Math.min(1, left / Math.max(buff.total, 0.001));
  const late = buff.slot !== null && left <= warn;
  return share < WARN_SHARE || late ? WARN : INK;
}

/** Whole seconds left, or a dash once gone. */
function seconds(buff: BuffTimer, now: number) {
  const left = remaining(buff, now);
  return left === null || left <= 0 ? "—" : String(Math.ceil(left));
}

/** SVG filters painting an icon's shape one flat colour, keyed by colour. */
const TINTS = { [INK]: "tint-ink", [WARN]: "tint-warn", [DOWN]: "tint-down" };

function Defs() {
  return (
    <defs>
      {/* the HUD's softness: a faint glow in each mark's own colour under a slight blur */}
      <filter
        id="hud"
        x="-30%"
        y="-30%"
        width="160%"
        height="160%"
        colorInterpolationFilters="sRGB"
      >
        <feGaussianBlur in="SourceGraphic" stdDeviation={1.2} result="glow" />
        <feComponentTransfer in="glow" result="faint">
          <feFuncA type="linear" slope={0.3} />
        </feComponentTransfer>
        <feGaussianBlur in="SourceGraphic" stdDeviation={0.4} result="soft" />
        <feMerge>
          <feMergeNode in="faint" />
          <feMergeNode in="soft" />
        </feMerge>
      </filter>
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

/** A ring round one ability's dot, lit clockwise from the top for the share of its buff left
 * over a faint track, the lit part edged dark so it holds on bright scenes. */
function Ring({
  buff,
  x,
  now,
  warn,
}: {
  buff: BuffTimer;
  x: number;
  now: number;
  warn: number;
}) {
  const left = remaining(buff, now) ?? 0;
  const colour = colourOf(buff, now, warn);
  const late = colour !== INK;
  const width = late ? RING_WARN_WIDTH : RING_WIDTH;
  const lit = left <= 0 ? 1 : Math.min(1, left / Math.max(buff.total, 0.001));
  const pulse =
    left <= 0
      ? 0.75 + 0.25 * Math.cos((now / 1000) * PULSE_RATE * 2 * Math.PI)
      : 1;
  const arc = {
    cx: x,
    cy: DOT_Y,
    r: RING_RADIUS,
    fill: "none",
    pathLength: 1,
    strokeDasharray: `${lit} 1`,
    transform: `rotate(-90 ${x} ${DOT_Y})`,
  };
  return (
    <g>
      <circle
        cx={x}
        cy={DOT_Y}
        r={RING_RADIUS}
        fill="none"
        stroke="black"
        strokeWidth={width + 1.6}
        opacity={0.15}
      />
      <circle
        cx={x}
        cy={DOT_Y}
        r={RING_RADIUS}
        fill="none"
        stroke="white"
        strokeWidth={width}
        opacity={0.16}
      />
      <circle
        {...arc}
        stroke="black"
        strokeWidth={width + (late ? 2 : 1.2)}
        opacity={late ? 0.65 : 0.55}
      />
      <circle {...arc} stroke={colour} strokeWidth={width} opacity={pulse} />
    </g>
  );
}

/** A label beside seconds, shortened so it stays on the strip. */
function shortLabel(label: string) {
  return label.length > NOTE_CHARS
    ? `${label.slice(0, NOTE_CHARS - 1).trimEnd()}…`
    : label;
}

/** Seconds centred on `x`, their middle on `y`, with `note` naming what they count on the far
 * side from `anchor`, so the number stays put whatever the note's length. */
function Seconds({
  text,
  x,
  y,
  colour,
  fontSize,
  anchor = "middle",
  note,
}: {
  text: string;
  x: number;
  y: number;
  colour: string;
  fontSize: number;
  anchor?: "middle" | "start" | "end";
  note?: string;
}) {
  const label = note ? (
    <tspan fontSize={fontSize * NOTE_TEXT} fontWeight={500} opacity={0.9}>
      {shortLabel(note)}
    </tspan>
  ) : null;
  const gap = <tspan fontSize={fontSize * NOTE_TEXT}> </tspan>;
  return (
    <text
      x={x}
      y={y + (fontSize * DIGIT_HEIGHT) / 2}
      fill={colour}
      fontFamily={FONT}
      fontWeight={700}
      textAnchor={anchor}
      dominantBaseline="alphabetic"
      fontSize={fontSize}
      {...TEXT_OUTLINE}
    >
      {label && anchor === "end" && (
        <>
          {label}
          {gap}
        </>
      )}
      <tspan>{text}</tspan>
      {label && anchor !== "end" && (
        <>
          {gap}
          {label}
        </>
      )}
    </text>
  );
}

/** The game's icon for a buff, centred on `x`, `y`, tinted like its seconds. */
function Icon({
  name,
  x,
  y,
  tint,
  size = ICON_SIZE,
}: {
  name: string;
  x: number;
  y: number;
  tint: string;
  size?: number;
}) {
  const src = useItemImageSrc(name);
  if (!src) {
    return null;
  }
  return (
    <image
      href={src}
      x={x - size / 2}
      y={y - size / 2}
      width={size}
      height={size}
      filter={`url(#${tint})`}
    />
  );
}

/** One ability's several buffs as rows down from `TRAY_TOP`, icons in a column at `x` and
 * seconds beyond them on `side` (-1 left, 1 right), the marked buff larger, lit and ticked. */
function Tray({
  buffs,
  lead,
  x,
  side,
  now,
  warn,
  fontSize,
}: {
  buffs: BuffTimer[];
  lead: BuffTimer;
  x: number;
  side: number;
  now: number;
  warn: number;
  fontSize: number;
}) {
  return (
    <g>
      {buffs.map((buff, index) => {
        const marked = buff.id === lead.id;
        const y = TRAY_TOP + index * TRAY_ROW;
        const size = marked ? TRAY_LEAD_ICON : TRAY_ICON;
        const colour = colourOf(buff, now, warn);
        const tickX = x - side * (TRAY_LEAD_ICON / 2 + 2.5);
        return (
          <g key={buff.id} opacity={marked ? 1 : TRAY_REST_OPACITY}>
            {marked && (
              <line
                x1={tickX}
                x2={tickX}
                y1={y - TICK_HEIGHT / 2}
                y2={y + TICK_HEIGHT / 2}
                stroke={INK}
                strokeWidth={TICK_WIDTH}
                strokeLinecap="round"
              />
            )}
            {buff.icon ? (
              <Icon
                name={buff.icon}
                x={x}
                y={y}
                tint={TINTS[colour]}
                size={size}
              />
            ) : (
              <circle
                cx={x}
                cy={y}
                r={TRAY_DOT * (size / TRAY_ICON)}
                fill={colour}
                stroke="black"
                strokeOpacity={0.7}
                strokeWidth={0.8}
              />
            )}
            <Seconds
              text={seconds(buff, now)}
              x={x + side * (TRAY_LEAD_ICON / 2 + 2)}
              y={y}
              colour={colour}
              fontSize={fontSize * (marked ? 1 : TRAY_REST_TEXT)}
              anchor={side < 0 ? "end" : "start"}
              note={buff.icon ? undefined : buff.label}
            />
          </g>
        );
      })}
    </g>
  );
}

/** Of several buffs, the running one that ends first. */
function lowest(buffs: BuffTimer[], now: number) {
  const running = buffs.filter((buff) => (remaining(buff, now) ?? 0) > 0);
  const end = (buff: BuffTimer) => buff.ends_at ?? Number.POSITIVE_INFINITY;
  return running.length === 0
    ? null
    : running.reduce((best, buff) => (end(buff) < end(best) ? buff : best));
}

/** The buff an ability's ring shows: the one its widget shows largest, else the running one
 * that ends first, else any. */
function marked(buffs: BuffTimer[], now: number) {
  return (
    buffs.find((buff) => buff.rank === 0) ?? lowest(buffs, now) ?? buffs[0]
  );
}

/** One ability's buffs in fixed places. The game names an ability's own widget's buffs by
 * their place in it, which is the order cycling goes through them, so sorting by name keeps
 * that order whichever is shown largest. */
function trayOrder(buffs: BuffTimer[]) {
  return [...buffs].sort((a, b) =>
    a.id.localeCompare(b.id, undefined, { numeric: true }),
  );
}

/** A meter's fill, as a share of its length. */
function share(value: number, of: number) {
  return of > 0 ? Math.min(1, Math.max(0, value / of)) : 0;
}

/** A colour the HUD draws a line in, `0xRRGGBB`, as CSS. */
function colour(rgb: number | null, unread = UNREAD) {
  return rgb === null ? unread : `#${rgb.toString(16).padStart(6, "0")}`;
}

/** The meters' curve: a shallow smile under the dots. */
function meterY(x: number) {
  return DOT_Y + METER_DROP - METER_CURVE * ((x - CENTRE) / METER_END) ** 2;
}

/** One side's meter as a path from its outer end in to the gap, so a dash from its start fills
 * from the outer end. `side` is -1 for the left, 1 for the right; `lift` raises it. */
function meterPath(side: number, lift = 0) {
  const steps = 16;
  const points = Array.from({ length: steps + 1 }, (_, index) => {
    const x =
      CENTRE + side * (METER_END - ((METER_END - METER_GAP) * index) / steps);
    return `${x} ${meterY(x) - lift}`;
  });
  return `M${points.join("L")}`;
}

/** One meter: a faint track on a dark bed, lit from its outer end for `fill`, edged dark. */
function Meter({
  side,
  fill,
  stroke,
  width = METER_WIDTH,
  edge = 0.7,
}: {
  side: number;
  fill: number;
  stroke: string;
  width?: number;
  edge?: number;
}) {
  const d = meterPath(side);
  const lit = { d, fill: "none", pathLength: 1, strokeDasharray: `${fill} 1` };
  return (
    <g>
      <path
        d={d}
        fill="none"
        stroke="black"
        strokeWidth={width + 1.8}
        opacity={0.4}
      />
      <path
        d={d}
        fill="none"
        stroke="white"
        strokeWidth={width}
        opacity={0.3}
      />
      {fill > 0 && (
        <>
          <path
            {...lit}
            stroke="black"
            strokeWidth={width + 1.4}
            opacity={edge}
          />
          <path {...lit} stroke={stroke} strokeWidth={width} opacity={0.9} />
        </>
      )}
    </g>
  );
}

/** A thin line over the left meter, for overshields or overguard, `lift` above its middle. */
function OverLine({
  fill,
  stroke,
  lift,
}: {
  fill: number;
  stroke: string;
  lift: number;
}) {
  return (
    fill > 0 && (
      <path
        d={meterPath(-1, lift)}
        fill="none"
        stroke={stroke}
        strokeWidth={OVER_WIDTH}
        pathLength={1}
        strokeDasharray={`${fill} 1`}
        opacity={0.9}
      />
    )
  );
}

/** Shields on the left, or health once they break, and energy on the right, with overshields
 * and overguard as lines over the shields. */
function VitalMeters({ vitals }: { vitals: Vitals }) {
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
  const broken = vitals.max_shield <= 0 || shield <= 0;
  const quiet = (broken ? health : shield) > FULL && energy > FULL;
  const lift = METER_WIDTH / 2 + 1.2;
  return (
    <g opacity={quiet && overshield <= 0 && overguard <= 0 ? FULL_OPACITY : 1}>
      {broken ? (
        <Meter
          side={-1}
          fill={health}
          stroke={colour(vitals.health_colour, HEALTH)}
          width={HEALTH_WIDTH}
          edge={0.75}
        />
      ) : (
        <Meter
          side={-1}
          fill={shield}
          stroke={colour(vitals.shield_colour, SHIELD)}
        />
      )}
      <Meter side={1} fill={energy} stroke={ENERGY} />
      <OverLine
        fill={overshield}
        stroke={colour(vitals.overshield_colour)}
        lift={lift}
      />
      <OverLine
        fill={overguard}
        stroke={colour(vitals.overguard_colour)}
        lift={lift + (overshield > 0 ? OVER_WIDTH + 0.6 : 0)}
      />
    </g>
  );
}

/** Of the other buffs, the one about to run out, its icon and seconds right of the dots. */
function UrgentBuff({
  buffs,
  x = OTHER_X,
  y = DOT_Y,
  now,
  warn,
  fontSize,
}: {
  buffs: BuffTimer[];
  x?: number;
  y?: number;
  now: number;
  warn: number;
  fontSize: number;
}) {
  const buff = lowest(
    buffs.filter((buff) => colourOf(buff, now, warn) === WARN),
    now,
  );
  if (!buff) {
    return null;
  }
  return (
    <g opacity={OTHER_OPACITY}>
      {buff.icon && <Icon name={buff.icon} x={x} y={y} tint={TINTS[WARN]} />}
      <Seconds
        text={seconds(buff, now)}
        x={x + ICON_SIZE / 2 + 1.5}
        y={y}
        colour={WARN}
        fontSize={fontSize}
        anchor="start"
        note={buff.icon ? undefined : buff.label}
      />
    </g>
  );
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
  const slots = Array.from({ length: SLOTS }, (_, index) => ({
    x: slotX(index - (SLOTS - 1) / 2),
    side: index < SLOTS / 2 ? -1 : 1,
    buffs: buffs.filter((buff) => buff.slot === index + 1),
  }));
  /** Trays, for abilities with several buffs: the running ones, and the marked one even once
   * it has ended, in fixed places, stepping outward on their side. */
  const outward = new Map<number, number>();
  const trays = slots.flatMap(({ x, side, buffs }) => {
    if (buffs.length < 2) {
      return [];
    }
    const lead = marked(buffs, now);
    if (!lead) {
      return [];
    }
    const rows = trayOrder(buffs).filter(
      (buff) => buff.id === lead.id || (remaining(buff, now) ?? 0) > 0,
    );
    if (rows.length < 2) {
      return [];
    }
    const step = outward.get(side) ?? 0;
    outward.set(side, step + 1);
    return [
      {
        key: x,
        lead,
        rows,
        side,
        x: CENTRE + side * (TRAY_OUT + step * TRAY_STEP),
      },
    ];
  });
  /** The urgent other buff goes under the innermost tray on the right, if there is one. */
  const right = trays.find((tray) => tray.side > 0);
  const others = buffs.filter(
    (buff) => buff.slot === null || buff.slot > SLOTS,
  );
  /** The first row under the rings: under the meters when they show. */
  const below =
    (vitals
      ? DOT_Y + METER_DROP + HEALTH_WIDTH / 2
      : DOT_Y + RING_RADIUS + RING_WARN_WIDTH) + ROW_SPACE;

  return (
    <svg viewBox={`0 0 ${SIZE} ${SIZE}`} className="size-full">
      <Defs />
      <g transform={`translate(0 ${style.offset}) ${hudTransform(layout)}`}>
        <g opacity={opacity / 100} filter="url(#hud)">
          {vitals && <VitalMeters vitals={vitals} />}
          {slots.map(({ x, buffs }) => {
            const shown = marked(buffs, now);
            if (!shown) {
              return null;
            }
            const left = remaining(shown, now) ?? 0;
            /** Every slot's late seconds on one line, so two never stagger into one number;
             * a tray shows its own. */
            const late =
              !trays.some((tray) => tray.key === x) &&
              colourOf(shown, now, warn) === WARN &&
              left > 0;
            return (
              <g key={x}>
                <Ring buff={shown} x={x} now={now} warn={warn} />
                {late && (
                  <Seconds
                    text={seconds(shown, now)}
                    x={x}
                    y={below + fontSize / 2}
                    colour={WARN}
                    fontSize={fontSize}
                  />
                )}
              </g>
            );
          })}
          {trays.map((tray) => (
            <Tray
              key={tray.key}
              buffs={tray.rows}
              lead={tray.lead}
              x={tray.x}
              side={tray.side}
              now={now}
              warn={warn}
              fontSize={fontSize}
            />
          ))}
          <UrgentBuff
            buffs={others}
            x={right ? right.x : OTHER_X}
            y={
              right
                ? TRAY_TOP + right.rows.length * TRAY_ROW + URGENT_BREAK
                : DOT_Y
            }
            now={now}
            warn={warn}
            fontSize={fontSize * OTHER_TEXT}
          />
        </g>
      </g>
    </svg>
  );
}
