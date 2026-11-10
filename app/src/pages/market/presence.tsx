import { useEffect } from "react";
import { OptionSelect } from "@/components/option-select";
import { Badge } from "@/components/ui/badge";
import { api, logError, reportError } from "@/lib/bridge";
import type { FilterOption } from "@/lib/filters";
import { usePresenceStore } from "@/stores/presence-store";
import type { MarketPresenceChoice, UserStatus } from "@/types";

const STATUS_LABELS: Record<UserStatus, string> = {
  offline: "Offline",
  invisible: "Offline",
  online: "Online",
  ingame: "In game",
};

const CHOICE_OPTIONS: FilterOption<MarketPresenceChoice>[] = [
  { value: "untouched", label: "Status untouched" },
  { value: "invisible", label: STATUS_LABELS.invisible },
  { value: "online", label: STATUS_LABELS.online },
  { value: "ingame", label: STATUS_LABELS.ingame },
  { value: "follow_game", label: "Follow the game" },
];

export function LiveStatusBadge() {
  const live = usePresenceStore((state) => state.presence.live);
  if (live === null) {
    return <Badge variant="muted">Status unknown</Badge>;
  }

  const online = live === "online" || live === "ingame";
  return (
    <Badge variant={online ? "accent" : "muted"}>{STATUS_LABELS[live]}</Badge>
  );
}

export function PresenceControl() {
  const presence = usePresenceStore((state) => state.presence);
  const setPresence = usePresenceStore((state) => state.setPresence);

  useEffect(() => {
    async function load() {
      try {
        setPresence(await api.marketPresence());
      } catch (error) {
        logError("Reading the market presence", error);
      }
    }
    load();
  }, [setPresence]);

  const choose = async (choice: MarketPresenceChoice) => {
    try {
      await api.marketSetPresence(choice);
    } catch (error) {
      reportError(error);
    }
  };

  return (
    <OptionSelect
      label="warframe.market status"
      value={presence.choice}
      options={CHOICE_OPTIONS}
      onChange={choose}
      size="sm"
    />
  );
}
