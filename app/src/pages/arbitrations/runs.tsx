import type { ColumnDef } from "@tanstack/react-table";
import type { VariantProps } from "class-variance-authority";
import { Activity, FileUp, RefreshCw } from "lucide-react";
import { useMemo, useState } from "react";
import { ArbitrationRunFigures } from "@/components/arbitration-run-figures";
import { DataTable } from "@/components/data-table";
import { Figure } from "@/components/figure";
import { FilterGrid } from "@/components/filter-grid";
import { FilterSelect } from "@/components/filter-select";
import { ErrorNote, Section, Surface, TableSkeleton } from "@/components/page";
import { Badge, type badgeVariants } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Hint } from "@/components/ui/hint";
import { useAsyncData } from "@/hooks/use-async-data";
import { useFilters } from "@/hooks/use-filters";
import {
  earningsSummary,
  elapsedText,
  liveElapsedSecs,
  rateText,
  runDate,
  runRole,
  runTitle,
  STATUS_LABEL,
} from "@/lib/arbitration-runs";
import {
  type ArbitrationRunRoleFilter,
  nodeValues,
  slotMission,
  slotPlace,
} from "@/lib/arbitrations";
import { api, reportError } from "@/lib/bridge";
import { type FilterOption, plainOptions } from "@/lib/filters";
import { counted, num } from "@/lib/format";
import { notify } from "@/lib/toast";
import type {
  ArbitrationRun,
  ArbitrationRunSource,
  ArbitrationRunStatus,
} from "@/types";
import { HeroHeading } from "./schedule";

const STATUS_VARIANT: Record<
  ArbitrationRunStatus,
  VariantProps<typeof badgeVariants>["variant"]
> = {
  recording: "accent",
  completed: "secondary",
  failed: "warning",
  aborted: "muted",
  ended: "muted",
  interrupted: "muted",
};

const STATUSES: FilterOption<ArbitrationRunStatus>[] = (
  Object.keys(STATUS_LABEL) as ArbitrationRunStatus[]
).map((status) => ({ value: status, label: STATUS_LABEL[status] }));

const ROLES: FilterOption<ArbitrationRunRoleFilter>[] = [
  { value: "host", label: "Host" },
  { value: "client", label: "Client" },
];

const SOURCES: FilterOption<ArbitrationRunSource>[] = [
  { value: "live", label: "Recorded" },
  { value: "import", label: "Imported" },
];

interface ArbitrationRunFilters {
  mission: string | null;
  status: ArbitrationRunStatus | null;
  role: ArbitrationRunRoleFilter | null;
  source: ArbitrationRunSource | null;
}

const COLUMNS: ColumnDef<ArbitrationRun>[] = [
  {
    id: "started_at",
    accessorFn: (run) => run.started_at ?? "",
    header: "Date",
    cell: ({ row }) => runDate(row.original),
  },
  {
    id: "mission",
    accessorFn: slotMission,
    header: "Mission",
  },
  {
    id: "node",
    accessorFn: slotPlace,
    header: "Node",
    meta: { wrap: true },
  },
  {
    id: "role",
    accessorFn: runRole,
    header: "Role",
  },
  {
    id: "duration",
    accessorFn: (run) => run.duration_secs,
    header: "Duration",
    meta: { numeric: true },
    cell: ({ row }) => elapsedText(row.original.duration_secs),
  },
  {
    id: "cycles",
    accessorFn: (run) => run.reward_cycles,
    header: "Cycles",
    meta: { numeric: true, label: "Reward cycles" },
    cell: ({ row }) => num(row.original.reward_cycles),
  },
  {
    id: "drones",
    accessorFn: (run) => run.drone_spawns,
    header: "Drones",
    meta: { numeric: true },
    cell: ({ row }) => num(row.original.drone_spawns),
  },
  {
    accessorKey: "vitus",
    header: "Vitus Essence",
    meta: { numeric: true },
    cell: ({ row }) => num(row.original.vitus),
  },
  {
    id: "status",
    accessorFn: (run) => run.status,
    header: "Status",
    cell: ({ row }) => (
      <ArbitrationRunStatusBadge status={row.original.status} />
    ),
  },
];

export function ArbitrationRunStatusBadge({
  status,
}: {
  status: ArbitrationRunStatus;
}) {
  return <Badge variant={STATUS_VARIANT[status]}>{STATUS_LABEL[status]}</Badge>;
}

export function ArbitrationLiveRunCard({ now }: { now: number }) {
  const { data: run } = useAsyncData(
    "arbitration-live-run",
    api.arbitrationLiveRun,
    ["arbitrationRunUpdated", "arbitrationRunsUpdated"],
  );

  if (!run) {
    return null;
  }

  return (
    <Surface className="gap-2 px-4 py-3">
      <HeroHeading icon={Activity} label="Live run">
        <ArbitrationRunStatusBadge status={run.status} />
      </HeroHeading>
      <span className="text-primary truncate text-lg font-bold">
        {slotMission(run)}
      </span>
      <span className="truncate text-sm">
        {slotPlace(run)}, {runRole(run).toLowerCase()}
      </span>
      <div className="flex flex-wrap gap-x-6 gap-y-2">
        <ArbitrationRunFigures
          run={run}
          elapsedSecs={liveElapsedSecs(run, now)}
        />
      </div>
    </Surface>
  );
}

function ArbitrationEarnings({ runs }: { runs: ArbitrationRun[] }) {
  const summary = earningsSummary(runs);

  if (summary.runs === 0) {
    return null;
  }

  return (
    <div className="flex flex-wrap items-end gap-x-6 gap-y-2">
      <Figure label="Runs counted" value={num(summary.runs)} />
      <Figure label="Vitus Essence" value={num(summary.vitus)} />
      <Figure
        label="Vitus Essence per hour"
        value={rateText(summary.perHour, 1)}
      />
      <Hint as="span">
        Shown runs with Vitus Essence, per hour from their total run time
      </Hint>
    </div>
  );
}

function ArbitrationRunTable({
  runs,
  onOpenReport,
}: {
  runs: ArbitrationRun[];
  onOpenReport: (run: ArbitrationRun) => void;
}) {
  const filters = useFilters<ArbitrationRunFilters>({
    mission: null,
    status: null,
    role: null,
    source: null,
  });
  const chosen = filters.values;
  const missions = useMemo(() => nodeValues(runs, "mission_type"), [runs]);
  const shown = runs.filter(
    (run) =>
      (chosen.mission === null || run.mission_type === chosen.mission) &&
      (chosen.status === null || run.status === chosen.status) &&
      (chosen.role === null || run.host === (chosen.role === "host")) &&
      (chosen.source === null || run.source === chosen.source),
  );

  return (
    <>
      <ArbitrationEarnings runs={shown} />
      <DataTable
        tableId="arbitrationRuns"
        columns={COLUMNS}
        data={shown}
        resetKey={JSON.stringify(chosen)}
        searchPlaceholder="Filter runs"
        searchValue={runTitle}
        initialSorting={[{ id: "started_at", desc: true }]}
        rowKey={(run) => run.id}
        emptyMessage={
          runs.length === 0
            ? "No run recorded yet. Runs are recorded while Merframe watches the game log. Refresh reads the current EE.log again, Import reads an older one."
            : "No run matches these filters."
        }
        filters={
          <FilterGrid activeFilters={filters.active} onClear={filters.clear}>
            <FilterSelect
              label="Mission"
              value={chosen.mission}
              options={plainOptions(missions)}
              onChange={(value) => filters.set("mission", value)}
            />
            <FilterSelect
              label="Status"
              value={chosen.status}
              options={STATUSES}
              onChange={(value) => filters.set("status", value)}
            />
            <FilterSelect
              label="Role"
              value={chosen.role}
              options={ROLES}
              onChange={(value) => filters.set("role", value)}
            />
            <FilterSelect
              label="Source"
              value={chosen.source}
              options={SOURCES}
              onChange={(value) => filters.set("source", value)}
            />
          </FilterGrid>
        }
        onRowClick={onOpenReport}
      />
    </>
  );
}

export function ArbitrationRunHistory({
  runs,
  error,
  onOpenReport,
}: {
  runs: ArbitrationRun[] | null;
  error: string | null;
  onOpenReport: (run: ArbitrationRun) => void;
}) {
  const [importing, setImporting] = useState(false);

  const importRuns = async (
    load: () => Promise<number | null>,
    nothingNew: string,
  ) => {
    setImporting(true);
    try {
      const imported = await load();
      if (imported === 0) {
        notify.info(nothingNew);
      } else if (imported !== null) {
        notify.success(`Imported ${counted(imported, "arbitration run")}`);
      }
    } catch (error) {
      reportError(error);
    } finally {
      setImporting(false);
    }
  };

  return (
    <Section
      title="Run History"
      description="Every arbitration read from the game log. Click a run for its report."
      action={
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            disabled={importing}
            onClick={() =>
              importRuns(
                api.refreshArbitrationRuns,
                "No new arbitration run in EE.log",
              )
            }
          >
            <RefreshCw className="size-4" />
            Refresh from EE.log
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={importing}
            onClick={() =>
              importRuns(
                api.importArbitrationLog,
                "No new arbitration run in that log",
              )
            }
          >
            <FileUp className="size-4" />
            Import a Log
          </Button>
        </div>
      }
    >
      <div className="flex flex-col gap-4">
        {error && <ErrorNote message={error} />}
        {runs ? (
          <ArbitrationRunTable runs={runs} onOpenReport={onOpenReport} />
        ) : (
          !error && <TableSkeleton />
        )}
      </div>
    </Section>
  );
}
