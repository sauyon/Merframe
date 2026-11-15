import { BellRing, ChevronDown } from "lucide-react";
import { useCallback, useState } from "react";
import { Section } from "@/components/page";
import { Button } from "@/components/ui/button";
import { CheckboxMenu } from "@/components/ui/checkbox-menu";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PositionPicker } from "@/components/ui/position-picker";
import { api, reportError } from "@/lib/bridge";
import { toggled } from "@/lib/filters";
import { notify } from "@/lib/toast";
import type {
  DiscordMirroredAlert,
  InGameToast,
  Settings,
  ToastPosition,
} from "@/types";
import { CheckboxRow, type Patch, SwitchRow } from "./row";

const TOAST_POSITIONS: { value: ToastPosition; label: string }[] = [
  { value: "top_left", label: "Top left" },
  { value: "top_centre", label: "Top centre" },
  { value: "top_right", label: "Top right" },
  { value: "bottom_left", label: "Bottom left" },
  { value: "bottom_centre", label: "Bottom centre" },
  { value: "bottom_right", label: "Bottom right" },
];

const DISCORD_MIRRORED: { value: DiscordMirroredAlert; label: string }[] = [
  { value: "conversation", label: "New conversations" },
  { value: "fissure", label: "Fissure alerts" },
  { value: "reward", label: "Reward alerts" },
  { value: "arbitration", label: "Arbitration alerts" },
  { value: "timer", label: "Cycle timers" },
];

const IN_GAME_TOASTS: { value: InGameToast; label: string }[] = [
  { value: "inventory", label: "Inventory changes" },
  { value: "trade", label: "Completed trades" },
  { value: "fissure", label: "Fissure alerts" },
  { value: "reward", label: "Reward alerts" },
  { value: "arbitration", label: "Arbitration alerts" },
  { value: "timer", label: "Timer alerts" },
  { value: "market_close", label: "Closed orders and auctions" },
];

function PositionSelect({
  id,
  label,
  value,
  disabled,
  onChange,
}: {
  id: string;
  label: string;
  value: ToastPosition;
  disabled?: boolean;
  onChange: (value: ToastPosition) => void;
}) {
  return (
    <div className="flex flex-col items-start gap-2">
      <Label htmlFor={id}>{label}</Label>
      <PositionPicker
        id={id}
        label={label}
        value={value}
        options={TOAST_POSITIONS}
        disabled={disabled}
        onChange={onChange}
      />
    </div>
  );
}

export function NotificationChannels({
  draft,
  patch,
}: {
  draft: Settings;
  patch: Patch;
}) {
  const [testing, setTesting] = useState(false);
  const handleTest = useCallback(async () => {
    setTesting(true);
    try {
      await api.testNotifications(draft);
      notify.success("Test notification sent");
    } catch (error) {
      reportError(error);
    } finally {
      setTesting(false);
    }
  }, [draft]);

  return (
    <Section title="Notification Channels">
      <div className="grid gap-x-8 gap-y-6 @3xl:grid-cols-2">
        <div className="flex flex-col gap-4">
          <SwitchRow
            checked={draft.toasts_enabled}
            onChange={(checked) => patch({ toasts_enabled: checked })}
            hint="Errors are always shown."
          >
            In-app toasts for confirmations and alerts
          </SwitchRow>
          <PositionSelect
            id="toast-position"
            label="Toast position"
            value={draft.toast_position}
            onChange={(value) => patch({ toast_position: value })}
          />
          <CheckboxRow
            checked={draft.toasts_in_game}
            disabled={!draft.toasts_enabled}
            onChange={(checked) => patch({ toasts_in_game: checked })}
            hint="Shows informative notifications also as an overlay in-game. Needs overlays enabled."
          >
            In-game notifications
          </CheckboxRow>
          <CheckboxMenu
            trigger={
              <>
                {IN_GAME_TOASTS.length - draft.toasts_in_game_muted.length} of{" "}
                {IN_GAME_TOASTS.length} shown in-game
                <ChevronDown className="size-4" />
              </>
            }
            className="self-start"
            disabled={!draft.toasts_enabled || !draft.toasts_in_game}
            items={IN_GAME_TOASTS.map(({ value, label }) => ({
              key: value,
              label,
              checked: !draft.toasts_in_game_muted.includes(value),
              onCheckedChange: (checked) =>
                patch({
                  toasts_in_game_muted: toggled(
                    draft.toasts_in_game_muted,
                    value,
                    !checked,
                  ),
                }),
            }))}
          />
          <PositionSelect
            id="toast-in-game-position"
            label="In-game notification position"
            value={draft.toasts_in_game_position}
            disabled={!draft.toasts_enabled || !draft.toasts_in_game}
            onChange={(value) => patch({ toasts_in_game_position: value })}
          />
          <SwitchRow
            checked={draft.windows_notifications_enabled}
            onChange={(checked) =>
              patch({ windows_notifications_enabled: checked })
            }
          >
            Desktop notifications
          </SwitchRow>
          <CheckboxRow
            checked={draft.sound_notifications_enabled}
            disabled={!draft.windows_notifications_enabled}
            onChange={(checked) =>
              patch({ sound_notifications_enabled: checked })
            }
            hint="Uses your desktop's notification sound."
          >
            Play a sound with them
          </CheckboxRow>
          <CheckboxRow
            checked={draft.notification_only_background}
            onChange={(checked) =>
              patch({ notification_only_background: checked })
            }
          >
            Only notify while Warframe is in the background
          </CheckboxRow>
          <Button
            variant="outline"
            size="sm"
            className="self-start"
            onClick={handleTest}
            pending={testing}
          >
            <BellRing className="size-4" />
            Test Notifications
          </Button>
        </div>
        <div className="flex flex-col gap-4">
          <SwitchRow
            checked={draft.discord_mirror_enabled}
            onChange={(checked) => patch({ discord_mirror_enabled: checked })}
          >
            Mirror to Discord
          </SwitchRow>
          <div className="flex flex-wrap gap-x-6 gap-y-2">
            {DISCORD_MIRRORED.map(({ value, label }) => (
              <CheckboxRow
                key={value}
                checked={draft.discord_mirrored_alerts.includes(value)}
                disabled={!draft.discord_mirror_enabled}
                onChange={(checked) =>
                  patch({
                    discord_mirrored_alerts: toggled(
                      draft.discord_mirrored_alerts,
                      value,
                      checked,
                    ),
                  })
                }
              >
                {label}
              </CheckboxRow>
            ))}
          </div>
          <Field id="webhook" label="Discord webhook URL">
            <Input
              id="webhook"
              value={draft.discord_webhook ?? ""}
              onChange={(e) =>
                patch({ discord_webhook: e.target.value || null })
              }
              placeholder="https://discord.com/api/webhooks/"
            />
          </Field>
          <Field
            id="template"
            label={
              <>
                Conversation notification,{" "}
                <span className="text-accent">{"{tenno}"}</span> is the sender's
                name
              </>
            }
          >
            <Input
              id="template"
              value={draft.discord_message_template}
              onChange={(e) =>
                patch({ discord_message_template: e.target.value })
              }
            />
          </Field>
        </div>
      </div>
    </Section>
  );
}
