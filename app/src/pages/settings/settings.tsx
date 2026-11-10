import { Save, TriangleAlert, Undo2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useLocation } from "react-router";
import { Page } from "@/components/page";
import { Button } from "@/components/ui/button";
import { api, reportError } from "@/lib/bridge";
import { notify } from "@/lib/toast";
import { scrollToSection } from "@/lib/utils";
import { useAppStore, useSettings } from "@/stores/app-store";
import type { Settings } from "@/types";
import { ArbitrationAlertSettingsSection } from "./arbitrations";
import { PricesAndData, StatsTabSetting } from "./data";
import { DisplaySetting } from "./display";
import { EventFeed } from "./feed";
import { FissureAlerts } from "./fissures";
import { NotificationChannels } from "./notifications";
import { WorldRewardAlertSettings } from "./rewards";
import { CycleTimers } from "./timers";

export function SettingsPage() {
  const stored = useSettings();
  const setStored = useAppStore((state) => state.setSettings);
  const [draft, setDraft] = useState(stored);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setDraft(stored);
  }, [stored]);

  const { hash } = useLocation();
  useEffect(() => {
    if (hash) {
      scrollToSection(hash.slice(1));
    }
  }, [hash]);

  const patch = useCallback(
    (next: Partial<Settings>) =>
      setDraft((current) => ({ ...current, ...next })),
    [],
  );

  const patchAlerts = useCallback(
    (next: Partial<Settings["alerts"]>) =>
      setDraft((current) => ({
        ...current,
        alerts: { ...current.alerts, ...next },
      })),
    [],
  );

  const handleSave = useCallback(async () => {
    setSaving(true);
    try {
      setStored(await api.settingsSet(draft));
      notify.success("Settings saved");
    } catch (error) {
      reportError(error);
    } finally {
      setSaving(false);
    }
  }, [draft, setStored]);

  const dirty = JSON.stringify(draft) !== JSON.stringify(stored);

  return (
    <Page title="Settings" quote="settings">
      <div className="sticky top-2 z-10 h-13">
        {dirty && (
          <div className="bg-card text-card-foreground border-border mx-auto flex w-fit items-center gap-3 rounded-lg border py-2 pr-2 pl-4 text-sm shadow-md">
            <TriangleAlert className="text-warning size-4" />
            <span className="font-medium">You have unsaved settings.</span>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setDraft(stored)}
              disabled={saving}
            >
              <Undo2 className="size-4" />
              Discard
            </Button>
            <Button size="sm" onClick={handleSave} disabled={saving}>
              <Save className="size-4" />
              Save
            </Button>
          </div>
        )}
      </div>

      <NotificationChannels draft={draft} patch={patch} />
      <div className="grid items-start gap-6 @7xl:grid-cols-2">
        <FissureAlerts draft={draft} patchAlerts={patchAlerts} />
        <CycleTimers draft={draft} patchAlerts={patchAlerts} />
        <WorldRewardAlertSettings draft={draft} patchAlerts={patchAlerts} />
        <ArbitrationAlertSettingsSection
          draft={draft}
          patchAlerts={patchAlerts}
        />
      </div>
      <PricesAndData draft={draft} patch={patch} />
      <StatsTabSetting draft={draft} patch={patch} />
      <DisplaySetting draft={draft} patch={patch} />
      <EventFeed draft={draft} patch={patch} />
    </Page>
  );
}
