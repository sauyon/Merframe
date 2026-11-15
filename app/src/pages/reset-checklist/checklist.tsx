import {
  Circle,
  CircleCheck,
  CircleDashed,
  Lock,
  type LucideIcon,
} from "lucide-react";
import { EmptyNote, Section } from "@/components/page";
import { Checkbox } from "@/components/ui/checkbox";
import { Hint } from "@/components/ui/hint";
import { NO_FILTER_MATCHES } from "@/lib/filters";
import { num } from "@/lib/format";
import { cn } from "@/lib/utils";
import type {
  ResetChecklistManualRowId,
  ResetChecklistProgress,
  ResetChecklistRow,
  ResetChecklistRowPart,
  ResetWindow,
} from "@/types";
import { ResetChecklistCountdown } from "./reset-countdown";

export type CheckResetChecklistRow = (
  id: ResetChecklistManualRowId,
  checked: boolean,
) => void;

type ResetChecklistRowStatus = "locked" | "unknown" | "done" | "open";

const STATUS_MARKS: Record<
  ResetChecklistRowStatus,
  { icon: LucideIcon; label: string; tone: string }
> = {
  locked: { icon: Lock, label: "Locked", tone: "text-muted-foreground" },
  unknown: {
    icon: CircleDashed,
    label: "Unknown",
    tone: "text-muted-foreground",
  },
  done: { icon: CircleCheck, label: "Done", tone: "text-primary" },
  open: { icon: Circle, label: "Not done", tone: "text-muted-foreground" },
};

function rowStatus(row: ResetChecklistRow): ResetChecklistRowStatus {
  if (row.locked) {
    return "locked";
  }

  if (row.progress.kind === "unknown") {
    return "unknown";
  }

  return row.done ? "done" : "open";
}

function ResetChecklistRowStatusMark({
  row,
  onCheck,
}: {
  row: ResetChecklistRow;
  onCheck: CheckResetChecklistRow;
}) {
  const { progress } = row;
  if (!row.locked && progress.kind === "manual") {
    return (
      <Checkbox
        aria-label={`${row.name} done`}
        checked={progress.checked}
        onCheckedChange={(checked) => onCheck(progress.id, checked === true)}
      />
    );
  }

  const { icon: Icon, label, tone } = STATUS_MARKS[rowStatus(row)];
  return <Icon role="img" aria-label={label} className={cn("size-4", tone)} />;
}

function countedText(done: number, total: number): string {
  return `${num(done)}/${num(total)}`;
}

function progressText(progress: ResetChecklistProgress): string | null {
  switch (progress.kind) {
    case "counted":
      return countedText(progress.done, progress.total);
    case "remaining":
      return `${num(progress.left)} left of ${num(progress.cap)}`;
    case "manual":
    case "yes_no":
      return null;
    case "unknown":
      return "No data yet";
  }
}

function ResetChecklistOpenParts({
  parts,
}: {
  parts: ResetChecklistRowPart[];
}) {
  const open = parts.filter((part) => !part.completed);
  if (open.length === 0) {
    return null;
  }

  return (
    <ul className="pl-7">
      {open.map((part) => (
        <li
          key={`${part.name}/${part.total}`}
          className="flex items-baseline gap-3"
        >
          <Hint as="span" className="min-w-0 flex-1">
            {part.name}
          </Hint>
          <Hint as="span" className="shrink-0">
            {countedText(part.done, part.total)}
          </Hint>
        </li>
      ))}
    </ul>
  );
}

function ResetChecklistItem({
  row,
  now,
  onCheck,
}: {
  row: ResetChecklistRow;
  now: number;
  onCheck: CheckResetChecklistRow;
}) {
  const progress = row.locked ? "Locked" : progressText(row.progress);

  return (
    <li className="py-1.5">
      <div className="flex items-center gap-3">
        <span className="flex size-4 shrink-0 items-center justify-center">
          <ResetChecklistRowStatusMark row={row} onCheck={onCheck} />
        </span>
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="flex flex-wrap items-baseline gap-x-2">
            <span
              className={cn(
                "text-sm",
                (row.done || row.locked) && "text-muted-foreground",
              )}
            >
              {row.name}
            </span>
            {row.detail && <Hint as="span">{row.detail}</Hint>}
          </span>
          {row.own_window && (
            <ResetChecklistCountdown window={row.own_window} now={now} />
          )}
        </span>
        {progress && (
          <span
            className={cn(
              "shrink-0 text-sm",
              (row.locked || row.progress.kind === "unknown") &&
                "text-muted-foreground text-xs",
            )}
          >
            {progress}
          </span>
        )}
      </div>
      <ResetChecklistOpenParts parts={row.parts} />
    </li>
  );
}

export function ResetChecklistSection({
  title,
  reset,
  rows,
  now,
  onCheck,
}: {
  title: string;
  reset: ResetWindow | null;
  rows: ResetChecklistRow[];
  now: number;
  onCheck: CheckResetChecklistRow;
}) {
  return (
    <Section
      title={title}
      action={reset && <ResetChecklistCountdown window={reset} now={now} />}
    >
      {rows.length === 0 ? (
        <EmptyNote>{NO_FILTER_MATCHES}</EmptyNote>
      ) : (
        <ul className="divide-y">
          {rows.map((row) => (
            <ResetChecklistItem
              key={row.id}
              row={row}
              now={now}
              onCheck={onCheck}
            />
          ))}
        </ul>
      )}
    </Section>
  );
}
