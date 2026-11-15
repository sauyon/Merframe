import type { ReactNode } from "react";
import { OptionSelect } from "@/components/option-select";
import { NumberInput } from "@/components/ui/number-input";
import { Switch } from "@/components/ui/switch";
import { toggled } from "@/lib/filters";
import type {
  DucateringCompleteSets,
  DucateringHidden,
  Settings,
} from "@/types";

type CompleteSetRule = "sell" | "keep" | "keep_above";

const SET_PLAT_WHEN_PICKED = 30;

function completeSetRule(sets: DucateringCompleteSets): CompleteSetRule {
  return typeof sets === "string" ? sets : "keep_above";
}

function SettingRow({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-6 py-2 text-sm">
      <span>{label}</span>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

export function DucateringFilters({
  settings,
  disabled,
  update,
}: {
  settings: Settings;
  disabled: boolean;
  update: (next: Partial<Settings>) => void;
}) {
  const hide = (kind: DucateringHidden) => (checked: boolean) =>
    update({
      ducatering_hidden: toggled(settings.ducatering_hidden, kind, checked),
    });
  const sets = settings.ducatering_complete_sets;
  const setPlat =
    typeof sets === "string" ? SET_PLAT_WHEN_PICKED : sets.keep_above.plat;
  const chooseCompleteSetRule = (rule: CompleteSetRule) =>
    update({
      ducatering_complete_sets:
        rule === "keep_above" ? { keep_above: { plat: setPlat } } : rule,
    });
  return (
    <div className="divide-border mt-2 flex flex-col divide-y">
      <SettingRow label="Sort the parts worth the most Ducats first">
        <Switch
          id="ducatering-most-ducats"
          checked={settings.ducatering_most_ducats_first}
          disabled={disabled}
          onCheckedChange={(checked) =>
            update({ ducatering_most_ducats_first: checked })
          }
        />
      </SettingRow>
      <SettingRow label="Hide parts you are selling on warframe.market">
        <Switch
          id="ducatering-listed"
          checked={settings.ducatering_hidden.includes("listed")}
          disabled={disabled}
          onCheckedChange={hide("listed")}
        />
      </SettingRow>
      <SettingRow label="Hide parts of weapons and Warframes you have not mastered yet">
        <Switch
          id="ducatering-unmastered"
          checked={settings.ducatering_hidden.includes("unmastered")}
          disabled={disabled}
          onCheckedChange={hide("unmastered")}
        />
      </SettingRow>
      <SettingRow label="Hide parts of vaulted sets, unless Varzia offers the set right now">
        <Switch
          id="ducatering-vaulted"
          checked={settings.ducatering_hidden.includes("vaulted")}
          disabled={disabled}
          onCheckedChange={hide("vaulted")}
        />
      </SettingRow>
      <SettingRow label="Parts of a set you own completely">
        <OptionSelect
          id="ducatering-complete-sets"
          label="Complete sets"
          value={completeSetRule(sets)}
          options={[
            { value: "sell", label: "Sell for Ducats" },
            { value: "keep", label: "Keep the set" },
            { value: "keep_above", label: "Keep when the set sells above" },
          ]}
          size="sm"
          disabled={disabled}
          onChange={chooseCompleteSetRule}
        />
        {typeof sets !== "string" && (
          <NumberInput
            id="ducatering-set-plat"
            value={sets.keep_above.plat}
            unit="Platinum"
            disabled={disabled}
            onChange={(plat) =>
              update({ ducatering_complete_sets: { keep_above: { plat } } })
            }
          />
        )}
      </SettingRow>
      <SettingRow label="Hide parts that sell for more than">
        <NumberInput
          id="ducatering-max-plat"
          value={settings.ducatering_max_plat}
          unit="Platinum"
          disabled={disabled}
          onChange={(value) => update({ ducatering_max_plat: value })}
        />
      </SettingRow>
      <SettingRow label="Hide parts worth fewer Ducats per Platinum than (0 turns this off)">
        <NumberInput
          id="ducatering-min-ducats-per-plat"
          value={settings.ducatering_min_ducats_per_plat}
          unit="Ducats per Platinum"
          disabled={disabled}
          onChange={(value) =>
            update({ ducatering_min_ducats_per_plat: value })
          }
        />
      </SettingRow>
    </div>
  );
}
