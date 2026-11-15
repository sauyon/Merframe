import {
  CalendarClock,
  Check,
  Gem,
  InfinityIcon,
  type LucideIcon,
  Radio,
  Tag,
} from "lucide-react";
import type { ReactNode } from "react";
import { Amount } from "@/components/currency";
import {
  archonIcon,
  GameIcon,
  type GameIconName,
} from "@/components/game-icon";
import { EmptyNote, Surface } from "@/components/page";
import { Hint } from "@/components/ui/hint";
import { num, secondsUntil } from "@/lib/format";
import { cn } from "@/lib/utils";
import { nodeLabel, timeLeft } from "@/lib/world";
import type {
  ArchonHunt,
  ArchonMission,
  BaroGroup,
  BaroStatus,
  ChallengeKind,
  Circuit,
  DarvoDeal,
  NightwaveSeason,
  PrimeResurgence,
  ResetWindow,
  Sortie,
  SortieMission,
} from "@/types";

const CHALLENGE_GROUPS: { value: ChallengeKind; label: string }[] = [
  { value: "daily", label: "Daily" },
  { value: "weekly", label: "Weekly" },
  { value: "eliteWeekly", label: "Elite weekly" },
];

export function InfoRow({
  label,
  hint,
  value,
  strong = false,
  owned,
}: {
  label: ReactNode;
  hint?: ReactNode;
  value: ReactNode;
  strong?: boolean;
  owned?: boolean | null;
}) {
  return (
    <div className="flex items-baseline justify-between gap-3 text-sm">
      <span className="flex min-w-0 flex-col">
        <span className="flex min-w-0 items-center gap-1.5">
          <span
            className={cn(
              "text-muted-foreground line-clamp-2",
              owned === false && "text-foreground",
            )}
          >
            {label}
          </span>
          {owned && (
            <Check
              aria-label="Owned"
              className="text-accent size-3.5 shrink-0"
            />
          )}
        </span>
        {hint && <Hint as="span">{hint}</Hint>}
      </span>
      <span
        className={cn(
          "text-foreground shrink-0 font-mono tabular-nums",
          owned && "text-muted-foreground",
          strong && "text-primary font-semibold",
        )}
      >
        {value}
      </span>
    </div>
  );
}

export function Panel({
  icon,
  glyph,
  title,
  children,
}: {
  icon?: LucideIcon;
  glyph?: GameIconName;
  title: string;
  children: ReactNode;
}) {
  const Icon = icon;
  return (
    <Surface>
      <div className="flex items-center gap-2 font-semibold">
        {glyph && <GameIcon name={glyph} size={22} alt="" />}
        {Icon && <Icon className="text-primary size-4" />}
        {title}
      </div>
      {children}
    </Surface>
  );
}

export function Group({
  label,
  children,
}: {
  label?: string | undefined;
  children: ReactNode;
}) {
  return (
    <div className="mt-2 flex flex-col gap-1.5">
      {label && (
        <Hint as="span" className="text-accent font-medium">
          {label}
        </Hint>
      )}
      {children}
    </div>
  );
}

function Boss({ name, faction }: { name: string; faction: string | null }) {
  return (
    <>
      {name}
      {faction && <span className="text-muted-foreground"> ({faction})</span>}
    </>
  );
}

function modifierName(mission: SortieMission | ArchonMission) {
  return "modifier_name" in mission ? mission.modifier_name : null;
}

function Missions({
  missions,
}: {
  missions: (SortieMission | ArchonMission)[];
}) {
  return (
    <ol className="flex flex-col gap-1.5">
      {missions.map((mission, index) => {
        const modifier = modifierName(mission);
        return (
          <li
            key={`${mission.node_id}-${mission.mission_type}-${modifier ?? ""}`}
            className="flex flex-col text-sm"
          >
            <span>
              <span className="text-muted-foreground mr-2 text-xs">
                {index + 1}
              </span>
              {mission.mission_name}
              <span className="text-muted-foreground">
                {" "}
                ({nodeLabel(mission.node_name, mission.node_id)})
              </span>
            </span>
            {modifier && (
              <span className="text-muted-foreground pl-4 text-xs">
                {modifier}
              </span>
            )}
          </li>
        );
      })}
    </ol>
  );
}

export function BaroPanel({
  baro,
  manifest,
  now,
}: {
  baro: BaroStatus | null;
  manifest: BaroGroup[];
  now: number;
}) {
  return (
    <Panel glyph="baro" title="Baro Ki'Teer">
      {baro === null ? (
        <EmptyNote>Not in the world state yet.</EmptyNote>
      ) : "Away" in baro ? (
        <InfoRow
          label="Arrives in"
          value={timeLeft(secondsUntil(baro.Away.arrives, now))}
          strong
        />
      ) : (
        <>
          <InfoRow
            label={`At ${nodeLabel(baro.Present.node_name, baro.Present.node_id)}`}
            value={timeLeft(secondsUntil(baro.Present.leaves, now))}
            strong
          />
          {manifest.map((group) => (
            <Group key={group.name} label={group.name}>
              {group.items.map((offer) => (
                <InfoRow
                  key={offer.name}
                  label={offer.name}
                  owned={offer.owned}
                  value={
                    offer.ducats === null ? (
                      "-"
                    ) : (
                      <Amount currency="ducats" plain>
                        {num(offer.ducats)}
                      </Amount>
                    )
                  }
                />
              ))}
            </Group>
          ))}
        </>
      )}
    </Panel>
  );
}

export function ResurgencePanel({
  resurgence,
  now,
}: {
  resurgence: PrimeResurgence | null;
  now: number;
}) {
  return (
    <Panel icon={Gem} title="Prime Resurgence">
      {resurgence === null ? (
        <EmptyNote>Varzia is not in the world state yet.</EmptyNote>
      ) : (
        <>
          <InfoRow
            label={`At ${nodeLabel(resurgence.node_name, resurgence.node_id)}`}
            value={timeLeft(secondsUntil(resurgence.ends, now))}
            strong
          />
          <div className="flex flex-col gap-1.5">
            {resurgence.offerings.map((offering) => (
              <InfoRow
                key={offering.name}
                label={offering.name}
                value={`${num(offering.regal_aya)} Regal Aya`}
              />
            ))}
          </div>
        </>
      )}
    </Panel>
  );
}

export function DarvoPanel({
  deals,
  now,
}: {
  deals: DarvoDeal[];
  now: number;
}) {
  return (
    <Panel icon={Tag} title="Darvo's Deal">
      {deals.length === 0 ? (
        <EmptyNote>Darvo is not running a deal right now.</EmptyNote>
      ) : (
        deals.map((deal) => (
          <div key={deal.name} className="flex flex-col gap-1.5">
            <InfoRow
              label={deal.name}
              value={
                <Amount currency="platinum" plain>
                  {num(deal.sale_price)}
                </Amount>
              }
              strong
            />
            <InfoRow
              label={
                <>
                  Was{" "}
                  <Amount currency="platinum" plain className="inline-flex">
                    {num(deal.original_price)}
                  </Amount>
                </>
              }
              value={`-${deal.discount_percent}%`}
            />
            <InfoRow
              label="Stock left"
              value={`${num(Math.max(0, deal.amount_total - deal.amount_sold))} of ${num(deal.amount_total)}`}
            />
            <InfoRow
              label="Ends in"
              value={timeLeft(secondsUntil(deal.expiry, now))}
            />
          </div>
        ))
      )}
    </Panel>
  );
}

export function ResetsPanel({
  daily,
  weekly,
  sortie,
  now,
}: {
  daily: ResetWindow | null;
  weekly: ResetWindow | null;
  sortie: Sortie | null;
  now: number;
}) {
  return (
    <Panel icon={CalendarClock} title="Reset Timers">
      {daily && (
        <InfoRow
          label="Daily reset (standing, Steel Path alerts)"
          value={timeLeft(secondsUntil(daily.expiry, now))}
        />
      )}
      {weekly && (
        <InfoRow
          label="Weekly reset (archons, Circuit, traders)"
          value={timeLeft(secondsUntil(weekly.expiry, now))}
        />
      )}
      {sortie && (
        <InfoRow
          label="Daily sortie"
          value={timeLeft(secondsUntil(sortie.expiry, now))}
        />
      )}
    </Panel>
  );
}

export function CircuitPanel({
  circuit,
  now,
}: {
  circuit: Circuit | null;
  now: number;
}) {
  return (
    <Panel icon={InfinityIcon} title="The Circuit">
      {circuit === null ? (
        <EmptyNote>
          This week's rotation is not in the world state yet.
        </EmptyNote>
      ) : (
        <>
          <InfoRow
            label="Rotates in"
            value={timeLeft(secondsUntil(circuit.rotates, now))}
            strong
          />
          <Group label="Normal">
            <span className="text-sm">{circuit.normal.join(", ")}</span>
          </Group>
          <Group label="Steel Path">
            <span className="text-sm">{circuit.hard.join(", ")}</span>
          </Group>
        </>
      )}
    </Panel>
  );
}

export function NightwavePanel({
  nightwave,
  now,
}: {
  nightwave: NightwaveSeason | null;
  now: number;
}) {
  return (
    <Panel icon={Radio} title="Nightwave">
      {nightwave === null ? (
        <EmptyNote>No season is in the world state yet.</EmptyNote>
      ) : (
        <>
          <InfoRow
            label={`${nightwave.name}, phase ${nightwave.phase + 1}`}
            value={timeLeft(secondsUntil(nightwave.expiry, now))}
            strong
          />
          {CHALLENGE_GROUPS.map(({ value: kind, label }) => {
            const group = nightwave.challenges.filter(
              (challenge) => challenge.kind === kind,
            );
            if (group.length === 0) {
              return null;
            }
            return (
              <Group key={kind} label={label}>
                {group.map((challenge) => (
                  <InfoRow
                    key={challenge.tag}
                    label={challenge.name}
                    hint={challenge.description}
                    value={timeLeft(secondsUntil(challenge.expiry, now))}
                  />
                ))}
              </Group>
            );
          })}
        </>
      )}
    </Panel>
  );
}

export function SortiePanel({
  sortie,
  now,
}: {
  sortie: Sortie | null;
  now: number;
}) {
  return (
    <Panel glyph="sortie" title="Sortie">
      {sortie ? (
        <>
          <InfoRow
            label={<Boss name={sortie.boss_name} faction={sortie.faction} />}
            value={timeLeft(secondsUntil(sortie.expiry, now))}
            strong
          />
          <Missions missions={sortie.missions} />
        </>
      ) : (
        <EmptyNote>No sortie right now.</EmptyNote>
      )}
    </Panel>
  );
}

export function ArchonPanel({
  hunt,
  now,
}: {
  hunt: ArchonHunt | null;
  now: number;
}) {
  return (
    <Panel glyph={hunt ? archonIcon(hunt.boss) : "archon"} title="Archon Hunt">
      {hunt ? (
        <>
          <InfoRow
            label={<Boss name={hunt.boss_name} faction={hunt.faction} />}
            value={timeLeft(secondsUntil(hunt.expiry, now))}
            strong
          />
          <Missions missions={hunt.missions} />
        </>
      ) : (
        <EmptyNote>No archon hunt right now.</EmptyNote>
      )}
    </Panel>
  );
}
