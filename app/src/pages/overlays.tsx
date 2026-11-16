import { type ReactNode, useEffect, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { BuffFilters } from "@/components/buff-filters";
import { DucateringFilters } from "@/components/ducatering-filters";
import { SelectField } from "@/components/option-select";
import {
  ArbitrationLiveSlot,
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
  | "overlay_ducatering_placement"
  | "overlay_arbitration_placement";

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
  const buffs = master && settings.overlay_buffs;
  const arbitration = master && settings.overlay_arbitration;

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
            <CommittedSlider
              id="overlay-size"
              label="Overlay size"
              value={settings.overlay_size}
              min={50}
              max={200}
              step={5}
              unit="%"
              disabled={!master}
              className="w-40"
              readoutClassName="w-10"
              onCommit={(value) => saveSettings({ overlay_size: value })}
            />
          </div>
          <div className="text-muted-foreground flex flex-col gap-1 text-sm">
            <p>
              &quot;Only while the game is active&quot; hides the overlay
              windows while Warframe is not the focused window.
            </p>
            <p>
              Overlays already grow with the game&apos;s resolution, so 100%
              looks the same at 1080p and 4K. The relic reward band always
              matches the game&apos;s reward cards, and the buff strip has its
              own size below.
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
          >
            <SwitchField
              id="buffs-abilities"
              label="Ability buffs"
              checked={settings.buffs_abilities}
              disabled={!buffs}
              onChange={(checked) => saveSettings({ buffs_abilities: checked })}
            />
            <SwitchField
              id="buffs-own"
              label="Arcane and mod buffs"
              checked={settings.buffs_own}
              disabled={!buffs}
              onChange={(checked) => saveSettings({ buffs_own: checked })}
            />
            <SwitchField
              id="buffs-squad"
              label="Squad buffs"
              checked={settings.buffs_squad}
              disabled={!buffs}
              onChange={(checked) => saveSettings({ buffs_squad: checked })}
            />
            <SwitchField
              id="buffs-vitals"
              label="Health and energy bars"
              checked={settings.buffs_vitals}
              disabled={!buffs}
              onChange={(checked) => saveSettings({ buffs_vitals: checked })}
            />
            <CommittedSlider
              id="buffs-warn"
              label="Warn at"
              value={settings.buffs_warn_seconds}
              min={0}
              max={60}
              step={1}
              unit="s"
              disabled={!buffs}
              className="w-32"
              readoutClassName="w-8"
              onCommit={(value) => saveSettings({ buffs_warn_seconds: value })}
            />
            <CommittedSlider
              id="buffs-scale"
              label="Size"
              value={settings.buffs_scale}
              min={50}
              max={200}
              step={5}
              unit="%"
              disabled={!buffs}
              className="w-32"
              readoutClassName="w-10"
              onCommit={(value) => saveSettings({ buffs_scale: value })}
            />
            <CommittedSlider
              id="buffs-offset"
              label="Height"
              value={-settings.buffs_offset}
              min={-60}
              max={60}
              step={1}
              disabled={!buffs}
              className="w-32"
              readoutClassName="w-8"
              onCommit={(value) => saveSettings({ buffs_offset: -value })}
            />
          </CardControls>
        }
        description={
          buffs
            ? "Ability timers on the ability dots below the reticle, other buffs beside them, read from the game. Warn at turns an ability's number amber with that many seconds left; Height moves the strip up or down."
            : OFF
        }
      >
        {buffs && (
          <BuffFilters
            hidden={settings.buffs_hidden}
            onChange={(hidden) => saveSettings({ buffs_hidden: hidden })}
          />
        )}
      </Section>

      <Section
        title="Live Arbitration"
        controls={
          <CardControls
            name="arbitration"
            enabled={settings.overlay_arbitration}
            disabled={!master}
            onEnabled={(enabled) =>
              saveSettings({ overlay_arbitration: enabled })
            }
          />
        }
        action={actions(
          "arbitration_live",
          "overlay_arbitration_placement",
          arbitration,
        )}
        description={
          arbitration
            ? "Shown while an Arbitration runs and for a moment after it ends, with the drone pace."
            : OFF
        }
      >
        <Focused trigger={arbitration ? overlays.arbitration : null}>
          <ArbitrationLiveSlot
            trigger={arbitration ? overlays.arbitration : null}
          />
        </Focused>
      </Section>
    </Page>
  );
}
