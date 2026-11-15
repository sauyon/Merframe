import { useState } from "react";
import { OptionSelect } from "@/components/option-select";
import { EmptyNote, Section } from "@/components/page";
import { SearchInput } from "@/components/search-input";
import { Label } from "@/components/ui/label";
import { useAsyncData } from "@/hooks/use-async-data";
import {
  ARBITRATION_ALERTS_SECTION_ID,
  ARBITRATION_LEAD_MINUTES,
  ARBITRATION_RELOAD_EVENTS,
  ARBITRATION_TIERS,
  nodeValues,
  slotMission,
  slotPlace,
} from "@/lib/arbitrations";
import { api } from "@/lib/bridge";
import { toggled } from "@/lib/filters";
import { counted } from "@/lib/format";
import type {
  ArbitrationAlertFilter,
  ArbitrationAlertSettings,
  ArbitrationNode,
  Settings,
} from "@/types";
import { CheckboxRow, type PatchAlerts, SwitchRow } from "./row";

const CHIP_GROUPS: {
  key: "mission_types" | "planets" | "factions";
  label: string;
  of: "mission_type" | "planet" | "faction";
}[] = [
  { key: "mission_types", label: "Mission types", of: "mission_type" },
  { key: "planets", label: "Planets", of: "planet" },
  { key: "factions", label: "Factions", of: "faction" },
];

function leadLabel(minutes: number): string {
  if (minutes === 0) {
    return "At rotation start";
  }

  if (minutes === 60) {
    return "1 hour before";
  }

  return `${counted(minutes, "minute")} before`;
}

const LEAD_OPTIONS = ARBITRATION_LEAD_MINUTES.map((minutes) => ({
  value: String(minutes),
  label: leadLabel(minutes),
}));

function ChosenLabel({ label, count }: { label: string; count: number }) {
  return (
    <Label>
      {label}
      <span className="text-muted-foreground font-normal">
        {count === 0 ? " (any)" : ` (${count} chosen)`}
      </span>
    </Label>
  );
}

function ChipRow({
  label,
  options,
  chosen,
  onChange,
}: {
  label: string;
  options: string[];
  chosen: string[];
  onChange: (next: string[]) => void;
}) {
  const shown = [...new Set([...options, ...chosen])];
  return (
    <div className="flex flex-col gap-2">
      <ChosenLabel label={label} count={chosen.length} />
      <div className="flex flex-wrap gap-x-6 gap-y-2">
        {shown.map((option) => (
          <CheckboxRow
            key={option}
            checked={chosen.includes(option)}
            onChange={(checked) => onChange(toggled(chosen, option, checked))}
          >
            {option}
          </CheckboxRow>
        ))}
      </div>
    </div>
  );
}

function NodePicker({
  nodes,
  chosen,
  onChange,
}: {
  nodes: ArbitrationNode[];
  chosen: string[];
  onChange: (next: string[]) => void;
}) {
  const [query, setQuery] = useState("");
  const search = query.trim().toLocaleLowerCase();
  const shown = nodes
    .filter(
      (node) =>
        !search ||
        `${slotPlace(node)} ${slotMission(node)}`
          .toLocaleLowerCase()
          .includes(search),
    )
    .sort(
      (a, b) =>
        Number(chosen.includes(b.node_id)) -
          Number(chosen.includes(a.node_id)) || a.node.localeCompare(b.node),
    );

  return (
    <div className="flex flex-col gap-2">
      <ChosenLabel label="Specific nodes" count={chosen.length} />
      <SearchInput
        value={query}
        onValueChange={setQuery}
        placeholder="Find a node"
        className="w-full"
      />
      {nodes.length === 0 ? (
        <EmptyNote>The forecast has not loaded yet.</EmptyNote>
      ) : (
        <ul className="flex max-h-64 flex-col gap-2 overflow-y-auto pr-2">
          {shown.map((node) => (
            <li key={node.node_id}>
              <CheckboxRow
                checked={chosen.includes(node.node_id)}
                onChange={(checked) =>
                  onChange(toggled(chosen, node.node_id, checked))
                }
                hint={slotMission(node)}
              >
                {slotPlace(node)}
              </CheckboxRow>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function ArbitrationAlertSettingsSection({
  draft,
  patchAlerts,
}: {
  draft: Settings;
  patchAlerts: PatchAlerts;
}) {
  const { data } = useAsyncData(
    "arbitrations",
    api.arbitrations,
    ARBITRATION_RELOAD_EVENTS,
  );
  const nodes = data?.schedule?.nodes ?? [];
  const arbitrations = draft.alerts.arbitrations;
  const filter = arbitrations.filter;
  const patch = (next: Partial<ArbitrationAlertSettings>) =>
    patchAlerts({ arbitrations: { ...arbitrations, ...next } });
  const patchFilter = (next: Partial<ArbitrationAlertFilter>) =>
    patch({ filter: { ...filter, ...next } });

  return (
    <Section
      id={ARBITRATION_ALERTS_SECTION_ID}
      title="Arbitration Alerts"
      description="Notifies about rotations that match every group below. An empty group matches everything."
    >
      <div className="flex flex-col gap-4">
        <SwitchRow
          checked={arbitrations.enabled}
          onChange={(checked) => patch({ enabled: checked })}
        >
          Notify about matching Arbitrations
        </SwitchRow>
        {CHIP_GROUPS.map((group) => (
          <ChipRow
            key={group.key}
            label={group.label}
            options={nodeValues(nodes, group.of)}
            chosen={filter[group.key]}
            onChange={(next) => patchFilter({ [group.key]: next })}
          />
        ))}
        <ChipRow
          label="Community tiers"
          options={ARBITRATION_TIERS}
          chosen={filter.tiers}
          onChange={(tiers) =>
            patchFilter({
              tiers: ARBITRATION_TIERS.filter((tier) => tiers.includes(tier)),
            })
          }
        />
        <NodePicker
          nodes={nodes}
          chosen={filter.node_ids}
          onChange={(node_ids) => patchFilter({ node_ids })}
        />
        <SwitchRow
          checked={filter.favourites_only}
          onChange={(checked) => patchFilter({ favourites_only: checked })}
          hint={
            arbitrations.favourite_nodes.length === 0
              ? "Star nodes on the Arbitrations page to add favourites."
              : `${counted(arbitrations.favourite_nodes.length, "node")} in your favourites.`
          }
        >
          Only favourite nodes
        </SwitchRow>
        <div className="flex flex-col gap-2">
          <Label htmlFor="arbitration-lead">Advance notice</Label>
          <OptionSelect
            id="arbitration-lead"
            label="Advance notice"
            value={String(arbitrations.lead_minutes)}
            options={LEAD_OPTIONS}
            onChange={(value) => patch({ lead_minutes: Number(value) })}
            className="max-w-64"
          />
        </div>
        <CheckboxRow
          checked={arbitrations.notify_on_start}
          disabled={arbitrations.lead_minutes === 0}
          onChange={(checked) => patch({ notify_on_start: checked })}
        >
          Also notify at rotation start
        </CheckboxRow>
        <SwitchRow
          checked={arbitrations.show_on_world_page}
          onChange={(checked) => patch({ show_on_world_page: checked })}
          hint="A compact card beside the cycle timers."
        >
          Show on the World page
        </SwitchRow>
      </div>
    </Section>
  );
}
