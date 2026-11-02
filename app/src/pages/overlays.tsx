import { type ReactNode, useEffect, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { DucateringFilters } from "@/components/ducatering-filters";
import { SelectField } from "@/components/option-select";
import {
  DucateringSlot,
  RecommendationSlot,
  RewardSlot,
  RivenSlot,
} from "@/components/overlay-slots";
import { Page, Section } from "@/components/page";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { PositionPicker } from "@/components/ui/position-picker";
import { Slider } from "@/components/ui/slider";
import { SwitchField } from "@/components/ui/switch";
import { api, reportError } from "@/lib/bridge";
import { cn } from "@/lib/utils";
import { useAppStore, useSettings } from "@/stores/app-store";
import { useOverlayStore } from "@/stores/overlay-store";
import type {
  OverlayKind,
  OverlayMode,
  OverlayPlacement,
  OverlaySupport,
  RecommendationRefinement,
} from "@/types";

const OFF = "Turned off.";

const OVERLAY_MODES: { value: OverlayMode; label: string }[] = [
  { value: "auto", label: "Over game if possible" },
  { value: "windows", label: "Always over the game" },
  { value: "tab", label: "Only the Overlays tab" },
];

const PLACEMENTS: { value: OverlayPlacement; label: string }[] = [
  { value: "top_left", label: "Top left" },
  { value: "top_right", label: "Top right" },
  { value: "bottom_left", label: "Bottom left" },
  { value: "bottom_right", label: "Bottom right" },
  { value: "centre", label: "Centre" },
];

type PlacementSetting =
  | "overlay_relic_reward_placement"
  | "overlay_relic_recommendation_placement"
  | "overlay_riven_placement"
  | "overlay_ducatering_placement";

const REFINEMENTS: { value: RecommendationRefinement; label: string }[] = [
  { value: "radiant", label: "Always Radiant" },
  { value: "owned", label: "Owned refinement" },
];

function renderNote(mode: OverlayMode, support: OverlaySupport | undefined) {
  if (mode === "tab") {
    return "Overlays render in this tab only.";
  }
  if (mode === "windows") {
    return "Overlays render as windows over the game and here.";
  }
  return support?.windows_possible
    ? "Overlays render as windows over the game and here."
    : "Overlay windows are unavailable in this session. Overlays render in this tab only.";
}

function Focused({
  trigger,
  children,
}: {
  trigger: unknown;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const node = ref.current;
    if (!trigger || !node) {
      return;
    }
    const reveal = () =>
      node.scrollIntoView({ behavior: "smooth", block: "nearest" });
    reveal();
    const observer = new ResizeObserver(reveal);
    observer.observe(node);
    return () => observer.disconnect();
  }, [trigger]);

  return <div ref={ref}>{children}</div>;
}

function CardControls({
  name,
  enabled,
  disabled,
  onEnabled,
  children,
}: {
  name: string;
  enabled: boolean;
  disabled: boolean;
  onEnabled: (enabled: boolean) => void;
  children?: ReactNode;
}) {
  return (
    <div className="mt-2 flex flex-wrap items-center gap-x-6 gap-y-2">
      <SwitchField
        id={`${name}-enabled`}
        label="Enabled"
        checked={enabled}
        disabled={disabled}
        onChange={onEnabled}
      />
      {children}
    </div>
  );
}

function CommittedSlider({
  id,
  label,
  value,
  min,
  max,
  step,
  unit = "",
  disabled,
  className,
  readoutClassName,
  onCommit,
}: {
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  unit?: string;
  disabled: boolean;
  className: string;
  readoutClassName: string;
  onCommit: (value: number) => void;
}) {
  const [dragged, setDragged] = useState<number | null>(null);
  const shown = dragged ?? value;
  return (
    <div className="flex items-center gap-3">
      <Label htmlFor={id} className="text-sm">
        {label}
      </Label>
      <Slider
        id={id}
        className={className}
        min={min}
        max={max}
        step={step}
        value={[shown]}
        disabled={disabled}
        onValueChange={([next = shown]) => setDragged(next)}
        onValueCommit={([next = shown]) => {
          setDragged(null);
          onCommit(next);
        }}
      />
      <span
        className={cn(
          "text-muted-foreground text-sm tabular-nums",
          readoutClassName,
        )}
      >
        {shown}
        {unit}
      </span>
    </div>
  );
}

export function OverlaysPage() {
  const settings = useSettings();
  const { status, saveSettings } = useAppStore(
    useShallow((state) => ({
      status: state.status,
      saveSettings: state.saveSettings,
    })),
  );
  const overlays = useOverlayStore((state) => state.overlays);

  const master = settings.overlays_enabled;
  const reward = master && settings.overlay_relic_reward;
  const recommendation = master && settings.overlay_relic_recommendation;
  const riven = master && settings.overlay_riven;
  const ducatering = master && settings.overlay_ducatering;

  const actions = (
    kind: OverlayKind,
    setting: PlacementSetting,
    enabled: boolean,
  ) => (
    <div className="flex shrink-0 items-start gap-3">
      <Button
        variant="outline"
        size="sm"
        disabled={!enabled}
        onClick={() => api.overlayTest(kind).catch(reportError)}
      >
        {overlays.preview === kind ? "Stop" : "Test"}
      </Button>
      <PositionPicker
        id={`${kind}-placement`}
        label="Placement"
        value={settings[setting]}
        options={PLACEMENTS}
        disabled={!master}
        onChange={(value) => saveSettings({ [setting]: value })}
      />
    </div>
  );

  return (
    <Page title="Overlays" quote="overlays">
      <Section
        title="Overlay Settings"
        description={renderNote(settings.overlay_mode, status?.overlay_support)}
      >
        <div className="flex flex-col gap-4">
          <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
            <SwitchField
              id="overlays-enabled"
              label="Enable overlays"
              checked={master}
              onChange={(checked) =>
                saveSettings({ overlays_enabled: checked })
              }
            />
            <SwitchField
              id="overlays-game-active"
              label="Only while the game is active"
              checked={settings.overlay_only_while_game_active}
              disabled={!master}
              onChange={(checked) =>
                saveSettings({ overlay_only_while_game_active: checked })
              }
            />
          </div>
          <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
            <SelectField
              id="overlay-mode"
              label="Where an overlay renders"
              inline
              value={settings.overlay_mode}
              options={OVERLAY_MODES}
              onChange={(value) => saveSettings({ overlay_mode: value })}
              size="sm"
              className="w-64"
            />
            <CommittedSlider
              id="overlay-opacity"
              label="Overlay opacity"
              value={settings.overlay_opacity}
              min={50}
              max={100}
              step={5}
              unit="%"
              disabled={!master}
              className="w-40"
              readoutClassName="w-10"
              onCommit={(value) => saveSettings({ overlay_opacity: value })}
            />
          </div>
          <div className="text-muted-foreground flex flex-col gap-1 text-sm">
            <p>
              &quot;Only while the game is active&quot; hides the overlay
              windows while Warframe is not the focused window.
            </p>
            <p>
              {status?.overlay_support.detail ??
                "The overlay support of this session is not known yet."}
            </p>
          </div>
        </div>
      </Section>

      <Section
        title="Relic Rewards"
        controls={
          <CardControls
            name="reward"
            enabled={settings.overlay_relic_reward}
            disabled={!master}
            onEnabled={(enabled) =>
              saveSettings({ overlay_relic_reward: enabled })
            }
          >
            <SwitchField
              id="reward-balance"
              label="Show your Platinum and Ducats"
              checked={settings.overlay_account_balance}
              disabled={!master}
              onChange={(checked) =>
                saveSettings({ overlay_account_balance: checked })
              }
            />
            <SwitchField
              id="reward-copy"
              label="Copy the rewards to the clipboard"
              checked={settings.copy_relic_rewards}
              onChange={(checked) =>
                saveSettings({ copy_relic_rewards: checked })
              }
            />
          </CardControls>
        }
        action={actions(
          "relic_reward",
          "overlay_relic_reward_placement",
          reward,
        )}
        description={
          reward ? "Shown while the reward screen is on display." : OFF
        }
      >
        <Focused trigger={reward ? overlays.reward : null}>
          <RewardSlot trigger={reward ? overlays.reward : null} />
        </Focused>
      </Section>

      <Section
        title="Relic Recommendation"
        controls={
          <CardControls
            name="recommendation"
            enabled={settings.overlay_relic_recommendation}
            disabled={!master}
            onEnabled={(enabled) =>
              saveSettings({ overlay_relic_recommendation: enabled })
            }
          >
            <SelectField
              id="recommendation-refinement"
              label="Values"
              inline
              value={settings.overlay_recommendation_refinement}
              options={REFINEMENTS}
              disabled={!master}
              onChange={(value) =>
                saveSettings({ overlay_recommendation_refinement: value })
              }
              size="sm"
              className="w-48"
            />
            <CommittedSlider
              id="recommendation-count"
              label="Relics shown"
              value={settings.overlay_recommendation_count}
              min={3}
              max={10}
              step={1}
              disabled={!master}
              className="w-28"
              readoutClassName="w-5"
              onCommit={(value) =>
                saveSettings({ overlay_recommendation_count: value })
              }
            />
          </CardControls>
        }
        action={actions(
          "relic_recommendation",
          "overlay_relic_recommendation_placement",
          recommendation,
        )}
        description={
          recommendation
            ? "Shown while the relic select screen is on display. Always Radiant ranks every relic as if you refined it first."
            : OFF
        }
      >
        <Focused trigger={recommendation ? overlays.recommendation : null}>
          <RecommendationSlot
            trigger={recommendation ? overlays.recommendation : null}
          />
        </Focused>
      </Section>

      <Section
        title="Riven"
        controls={
          <CardControls
            name="riven"
            enabled={settings.overlay_riven}
            disabled={!master}
            onEnabled={(enabled) => saveSettings({ overlay_riven: enabled })}
          />
        }
        action={actions("riven", "overlay_riven_placement", riven)}
        description={riven ? "Shown while you inspect or cycle a riven." : OFF}
      >
        <Focused trigger={riven ? overlays.riven : null}>
          <RivenSlot trigger={riven ? overlays.riven : null} />
        </Focused>
      </Section>

      <Section
        title="Ducatering"
        controls={
          <>
            <CardControls
              name="ducatering"
              enabled={settings.overlay_ducatering}
              disabled={!master}
              onEnabled={(enabled) =>
                saveSettings({ overlay_ducatering: enabled })
              }
            >
              <SwitchField
                id="ducatering-clipboard"
                label="Copy names to the clipboard"
                checked={settings.ducatering_clipboard}
                disabled={!master}
                onChange={(checked) =>
                  saveSettings({ ducatering_clipboard: checked })
                }
              />
            </CardControls>
            <DucateringFilters
              settings={settings}
              disabled={!master}
              update={saveSettings}
            />
          </>
        }
        action={actions(
          "ducatering",
          "overlay_ducatering_placement",
          ducatering,
        )}
        description={
          ducatering
            ? "Shown while the Ducat Kiosk is open. Filter changes only apply on new Kiosk screens."
            : OFF
        }
      >
        <Focused trigger={ducatering ? overlays.ducatering : null}>
          <DucateringSlot
            trigger={ducatering ? overlays.ducatering : null}
            clipboard={settings.ducatering_clipboard}
          />
        </Focused>
      </Section>

      <Section
        title="Buffs"
        controls={
          <CardControls
            name="buffs"
            enabled={settings.overlay_buffs}
            disabled={!master}
            onEnabled={(enabled) => saveSettings({ overlay_buffs: enabled })}
          />
        }
        description={
          master && settings.overlay_buffs
            ? "Countdown bars under the ability dots below the reticle, one fixed slot per ability, read from the game."
            : OFF
        }
      >
        {null}
      </Section>
    </Page>
  );
}
