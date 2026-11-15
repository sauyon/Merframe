import { Check } from "lucide-react";
import { Fragment } from "react";
import { Badge } from "@/components/ui/badge";
import { Dash } from "@/components/ui/dash";
import { Hint } from "@/components/ui/hint";
import { cn } from "@/lib/utils";
import type { AlternativeMatch, GoodRollView, StatMatch } from "@/types";

const GROUP_LABELS: Record<string, string> = { ELEMENT: "Any element" };

function statLabel(stat: StatMatch): string {
  return stat.name ?? GROUP_LABELS[stat.abbr] ?? stat.abbr;
}

function statKey(alternative: AlternativeMatch): string {
  return [...alternative.mandatory, ...alternative.optional]
    .map((stat) => stat.abbr)
    .join(" ");
}

function StatBadge({ stat }: { stat: StatMatch }) {
  return (
    <Badge
      variant="outline"
      className={cn(stat.matches ? "border-accent" : "text-muted-foreground")}
    >
      {statLabel(stat)}
      {stat.matches && (
        <Check className="text-accent" aria-label="On this roll" />
      )}
    </Badge>
  );
}

function Alternative({ alternative }: { alternative: AlternativeMatch }) {
  return (
    <div className="flex flex-wrap items-center gap-1 text-xs">
      {alternative.mandatory.map((stat) => (
        <StatBadge key={stat.abbr} stat={stat} />
      ))}
      {alternative.optional.length > 0 && (
        <>
          <span className="text-muted-foreground px-0.5">
            {alternative.mandatory.length > 0 ? "and any" : "any"}{" "}
            {alternative.optional_needed} of
          </span>
          {alternative.optional.map((stat) => (
            <StatBadge key={stat.abbr} stat={stat} />
          ))}
        </>
      )}
    </div>
  );
}

function OrRule() {
  return (
    <div className="flex items-center gap-2">
      <span className="bg-border h-px flex-1" />
      <span className="text-primary text-xs">or</span>
      <span className="bg-border h-px flex-1" />
    </div>
  );
}

export function GoodRollMarker({ view }: { view: GoodRollView | null }) {
  if (!view) {
    return <Dash />;
  }
  return (
    <Badge variant={view.matches ? "accent" : "muted"}>
      {view.matches ? "Yes" : "No"}
    </Badge>
  );
}

export function GoodRollBlock({
  view,
  className,
}: {
  view: GoodRollView;
  className?: string;
}) {
  return (
    <div className={cn("flex flex-col gap-2", className)}>
      <span className="text-primary text-sm">Desired positives</span>
      {view.alternatives.map((alternative, index) => (
        <Fragment key={statKey(alternative)}>
          {index > 0 && <OrRule />}
          <Alternative alternative={alternative} />
        </Fragment>
      ))}
      {view.accepted_bad.length > 0 && (
        <>
          <span className="text-primary text-sm">Desired negatives</span>
          <div className="flex flex-wrap items-center gap-1">
            {view.accepted_bad.map((stat) => (
              <StatBadge key={stat.abbr} stat={stat} />
            ))}
          </div>
        </>
      )}
    </div>
  );
}

export function GoodRollAttribution({
  attribution,
}: {
  attribution: string | null;
}) {
  if (!attribution) {
    return null;
  }
  return <Hint className="break-all">{attribution}</Hint>;
}
