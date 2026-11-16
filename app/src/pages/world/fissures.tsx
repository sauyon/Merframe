import { RelicTierIcon } from "@/components/game-icon";
import { EmptyNote, FramedList } from "@/components/page";
import { Badge } from "@/components/ui/badge";
import { Hint } from "@/components/ui/hint";
import { counted, secondsUntil } from "@/lib/format";
import { isRelicTier, RELIC_TIERS } from "@/lib/relics";
import { cn } from "@/lib/utils";
import { countdownTone, nodeLabel, timeLeft } from "@/lib/world";
import type { Fissure } from "@/types";

function FissureRow({ fissure, now }: { fissure: Fissure; now: number }) {
  const remaining = secondsUntil(fissure.expiry, now);
  return (
    <li className="flex items-center gap-3 px-3 py-2">
      <span className="min-w-0 flex-1 truncate text-sm">
        <span className="font-semibold">{fissure.mission_name}</span>
        <span className="text-muted-foreground">
          {" "}
          ({nodeLabel(fissure.node_name, fissure.node_id)})
        </span>
      </span>
      {fissure.steel_path && <Badge variant="secondary">Steel Path</Badge>}
      {fissure.is_storm && <Badge variant="secondary">Void Storm</Badge>}
      <span
        className={cn(
          "w-20 shrink-0 text-right font-mono text-sm",
          countdownTone(remaining),
        )}
      >
        {timeLeft(remaining)}
      </span>
    </li>
  );
}

export function FissuresByTier({
  fissures,
  now,
}: {
  fissures: Fissure[];
  now: number;
}) {
  const tiers: string[] = [...RELIC_TIERS];
  for (const fissure of fissures) {
    if (!tiers.includes(fissure.tier)) {
      tiers.push(fissure.tier);
    }
  }
  const grouped = tiers
    .map(
      (tier) =>
        [tier, fissures.filter((fissure) => fissure.tier === tier)] as const,
    )
    .filter(([, list]) => list.length > 0);

  if (grouped.length === 0) {
    return <EmptyNote>No fissures match this filter right now.</EmptyNote>;
  }
  return (
    <div className="grid items-start gap-3 @5xl:grid-cols-2">
      {grouped.map(([tier, list]) => (
        <FramedList
          key={tier}
          header={
            <>
              <RelicTierIcon tier={tier} size={22} />
              {isRelicTier(tier) && <Badge variant="secondary">{tier}</Badge>}
              <Hint as="span">{counted(list.length, "fissure")}</Hint>
            </>
          }
        >
          {list.map((fissure) => (
            <FissureRow
              key={`${fissure.node_id}-${fissure.expiry}-${fissure.steel_path}`}
              fissure={fissure}
              now={now}
            />
          ))}
        </FramedList>
      ))}
    </div>
  );
}
