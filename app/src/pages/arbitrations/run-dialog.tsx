import { ChevronDown, Trash2 } from "lucide-react";
import { useState } from "react";
import { ArbitrationRunFigures } from "@/components/arbitration-run-figures";
import { Figure } from "@/components/figure";
import { EmptyNote } from "@/components/page";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Field } from "@/components/ui/field";
import { Hint } from "@/components/ui/hint";
import { Input } from "@/components/ui/input";
import {
  boosterText,
  elapsedText,
  perMinute,
  rateText,
  runDate,
  runRole,
  runTitle,
} from "@/lib/arbitration-runs";
import { api, reportError } from "@/lib/bridge";
import { num } from "@/lib/format";
import { notify } from "@/lib/toast";
import type { ArbitrationRun, ArbitrationRunExportFormat } from "@/types";
import { ArbitrationRunChart } from "./run-chart";
import { ArbitrationRunPace } from "./run-pace";
import { ArbitrationRunStatusBadge } from "./runs";

const EXPORTS: { value: ArbitrationRunExportFormat; label: string }[] = [
  { value: "json", label: "Run data as JSON" },
  { value: "text", label: "Text report" },
  { value: "svg", label: "Report as an image" },
];

function vitusSource(run: ArbitrationRun): string {
  if (run.manual_vitus !== null) {
    return "Entered by you";
  }

  if (run.collected_vitus !== null) {
    return "Read from the inventory sync after the run";
  }

  return "Not read from the game for this run";
}

function ArbitrationRunVitusField({ run }: { run: ArbitrationRun }) {
  const [value, setValue] = useState(() => run.vitus?.toString() ?? "");
  const [saving, setSaving] = useState(false);
  const number = Number(value);
  const valid = value === "" || Number.isFinite(number);
  const entered = value === "" ? null : Math.max(0, Math.floor(number));

  const save = async () => {
    setSaving(true);
    try {
      await api.setArbitrationRunVitus(run.id, entered);
      notify.success("Vitus Essence saved");
    } catch (error) {
      reportError(error);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex flex-col gap-1">
      <Field id="arbitration-run-vitus" label="Vitus Essence" inline>
        <Input
          id="arbitration-run-vitus"
          type="number"
          min={0}
          step={1}
          value={value}
          onChange={(event) => setValue(event.target.value)}
          className="h-8 w-24 text-right"
        />
        <Button
          variant="outline"
          size="sm"
          disabled={!valid || entered === run.vitus}
          pending={saving}
          onClick={save}
        >
          Save
        </Button>
      </Field>
      <Hint>{vitusSource(run)}</Hint>
    </div>
  );
}

function ArbitrationRunOverview({ run }: { run: ArbitrationRun }) {
  const enemyRate = perMinute(run.enemy_spawns, run.duration_secs);
  return (
    <>
      <div className="grid grid-cols-2 gap-x-6 gap-y-3 @md:grid-cols-4">
        <ArbitrationRunFigures run={run} elapsedSecs={run.duration_secs} />
        <Figure label="Enemy spawns" value={num(run.enemy_spawns)} />
        <Figure label="Enemies per minute" value={rateText(enemyRate, 1)} />
        <Figure label="Most enemies alive" value={num(run.peak_live_agents)} />
      </div>
      <div className="flex flex-col gap-1">
        <Hint>Resource boosters at the start</Hint>
        {run.boosters.length > 0 ? (
          <ul className="flex flex-wrap gap-x-4 gap-y-1 text-sm">
            {run.boosters.map((booster) => (
              <li key={booster.kind}>
                {boosterText(booster, run.duration_secs)}
              </li>
            ))}
          </ul>
        ) : (
          <span className="text-sm">None</span>
        )}
      </div>
      {run.rewards && run.rewards.length > 0 && (
        <div className="flex flex-col gap-1">
          <Hint>Mission rewards</Hint>
          <ul className="flex flex-wrap gap-x-4 gap-y-1 text-sm">
            {run.rewards.map((reward) => (
              <li key={reward.item_type}>
                {reward.name}{" "}
                <span className="text-muted-foreground">
                  {num(reward.count)}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
      <ArbitrationRunVitusField run={run} />
      <ArbitrationRunPace run={run} />
      {run.samples.length > 1 ? (
        <ArbitrationRunChart run={run} />
      ) : (
        <EmptyNote>No spawn samples in this run.</EmptyNote>
      )}
      <ul className="flex flex-col gap-1">
        {run.notes.map((note) => (
          <li key={note}>
            <Hint>{note}</Hint>
          </li>
        ))}
      </ul>
    </>
  );
}

export function ArbitrationRunDialog({
  run,
  onClose,
}: {
  run: ArbitrationRun;
  onClose: () => void;
}) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  const exportRun = async (format: ArbitrationRunExportFormat) => {
    try {
      const path = await api.exportArbitrationRun(run.id, format);
      if (path !== null) {
        notify.success("Run exported", { description: path });
      }
    } catch (error) {
      reportError(error);
    }
  };

  const remove = async () => {
    setBusy(true);
    try {
      await api.deleteArbitrationRun(run.id);
      onClose();
      notify.success("Run deleted");
    } catch (error) {
      reportError(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent
        scrollable
        className="@container top-16 translate-y-0 sm:max-w-3xl"
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-3">
            {runTitle(run)}
            <ArbitrationRunStatusBadge status={run.status} />
          </DialogTitle>
          <DialogDescription>
            {runDate(run)}, {elapsedText(run.duration_secs)},{" "}
            {runRole(run).toLowerCase()}
          </DialogDescription>
        </DialogHeader>
        <ArbitrationRunOverview run={run} />
        <DialogFooter className="sm:justify-start">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" size="sm">
                Export
                <ChevronDown className="size-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
              {EXPORTS.map(({ value, label }) => (
                <DropdownMenuItem key={value} onSelect={() => exportRun(value)}>
                  {label}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
          <Button
            variant="destructive"
            size="sm"
            className="sm:ml-auto"
            disabled={busy}
            onClick={() => setConfirming(true)}
          >
            <Trash2 className="size-4" />
            Delete Run
          </Button>
        </DialogFooter>
      </DialogContent>
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title="Delete Run"
        description="This run is removed from the history, with anything you entered for it. This cannot be undone."
        confirmLabel="Delete"
        busy={busy}
        destructive
        onConfirm={remove}
      />
    </Dialog>
  );
}
