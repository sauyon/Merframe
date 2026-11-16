import { check } from "@tauri-apps/plugin-updater";
import { lazy, Suspense, useCallback, useEffect } from "react";
import { Navigate, Route, Routes } from "react-router";
import { useShallow } from "zustand/react/shallow";
import { AppSidebar } from "@/components/app-sidebar";
import { EventBridge } from "@/components/event-bridge";
import { MarketPanel } from "@/components/market-panel";
import { OverlayWindow } from "@/components/overlay-window";
import { StartupScreen } from "@/components/startup-screen";
import { StatusBar } from "@/components/status-bar";
import { Toaster } from "@/components/ui/sonner";
import { UpdateDialog } from "@/components/update-dialog";
import { useDisplayPreferences } from "@/hooks/use-display-preferences";
import { useLatestAttempt } from "@/hooks/use-latest-attempt";
import { api, logError } from "@/lib/bridge";
import { useAppStore, useSettings } from "@/stores/app-store";
import { useMarketItemsStore } from "@/stores/market-items-store";

const ResetChecklistPage = lazy(() =>
  import("@/pages/reset-checklist/reset-checklist").then((m) => ({
    default: m.ResetChecklistPage,
  })),
);
const AboutPage = lazy(() =>
  import("@/pages/about").then((m) => ({ default: m.AboutPage })),
);
const AnalyticsPage = lazy(() =>
  import("@/pages/analytics/analytics").then((m) => ({
    default: m.AnalyticsPage,
  })),
);
const FoundryPage = lazy(() =>
  import("@/pages/foundry/foundry").then((m) => ({ default: m.FoundryPage })),
);
const InventoryPage = lazy(() =>
  import("@/pages/inventory").then((m) => ({ default: m.InventoryPage })),
);
const MarketPage = lazy(() =>
  import("@/pages/market/market").then((m) => ({ default: m.MarketPage })),
);
const MasteryPage = lazy(() =>
  import("@/pages/mastery").then((m) => ({ default: m.MasteryPage })),
);
const OverlaysPage = lazy(() =>
  import("@/pages/overlays").then((m) => ({ default: m.OverlaysPage })),
);
const RelicPlannerPage = lazy(() =>
  import("@/pages/relic-planner/relic-planner").then((m) => ({
    default: m.RelicPlannerPage,
  })),
);
const ResourcesPage = lazy(() =>
  import("@/pages/resources/resources").then((m) => ({
    default: m.ResourcesPage,
  })),
);
const RivensPage = lazy(() =>
  import("@/pages/rivens/rivens").then((m) => ({ default: m.RivensPage })),
);
const ArbitrationsPage = lazy(() =>
  import("@/pages/arbitrations/arbitrations").then((m) => ({
    default: m.ArbitrationsPage,
  })),
);
const SettingsPage = lazy(() =>
  import("@/pages/settings/settings").then((m) => ({
    default: m.SettingsPage,
  })),
);
const StatsPage = lazy(() =>
  import("@/pages/stats").then((m) => ({ default: m.StatsPage })),
);
const WorldPage = lazy(() =>
  import("@/pages/world/world").then((m) => ({ default: m.WorldPage })),
);

function useBoot() {
  const { attempt, cancel } = useLatestAttempt();
  const { setReady, setBootError, setStatus, setSettings } = useAppStore(
    useShallow((state) => ({
      setReady: state.setReady,
      setBootError: state.setBootError,
      setStatus: state.setStatus,
      setSettings: state.setSettings,
    })),
  );

  const boot = useCallback(() => {
    setBootError(null);
    return attempt(() => Promise.all([api.gameStatus(), api.settingsGet()]), {
      loaded: ([status, settings]) => {
        setStatus(status);
        setSettings(settings);
        setReady(true);
      },
      failed: setBootError,
      quiet: true,
    });
  }, [attempt, setBootError, setReady, setSettings, setStatus]);

  useEffect(() => {
    boot();
    return cancel;
  }, [boot, cancel]);

  return boot;
}

function MainWindow() {
  const retry = useBoot();
  const { ready, bootError } = useAppStore(
    useShallow((state) => ({
      ready: state.ready,
      bootError: state.bootError,
    })),
  );
  const setItems = useMarketItemsStore((state) => state.setItems);

  useEffect(() => {
    if (!ready) {
      return;
    }
    api
      .marketItems()
      .then(setItems, (error) => logError("Market items", error));
  }, [ready, setItems]);

  return (
    <>
      <EventBridge />
      {ready ? <Shell /> : <StartupScreen error={bootError} onRetry={retry} />}
      <Toaster />
    </>
  );
}

function Shell() {
  const settings = useSettings();
  const setUpdate = useAppStore((state) => state.setUpdate);
  useDisplayPreferences(settings);

  const checkForUpdates = settings.check_for_updates;
  useEffect(() => {
    if (!checkForUpdates) {
      return;
    }

    api
      .updatesSupported()
      .then((supported) => (supported ? check() : null))
      .then(setUpdate, (error) => logError("Update check", error));
  }, [checkForUpdates, setUpdate]);

  return (
    <div className="flex h-full w-full overflow-hidden">
      <AppSidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <StatusBar />
        <main className="min-h-0 flex-1 overflow-y-scroll">
          <div className="@container">
            <Suspense fallback={null}>
              <Routes>
                <Route path="/" element={<Navigate to="/world" replace />} />
                <Route path="/world" element={<WorldPage />} />
                <Route
                  path="/reset-checklist"
                  element={<ResetChecklistPage />}
                />
                <Route path="/arbitrations" element={<ArbitrationsPage />} />
                <Route path="/foundry" element={<FoundryPage />} />
                <Route path="/inventory" element={<InventoryPage />} />
                <Route path="/relics" element={<RelicPlannerPage />} />
                <Route path="/rivens" element={<RivensPage />} />
                <Route path="/overlays" element={<OverlaysPage />} />
                <Route path="/mastery" element={<MasteryPage />} />
                <Route path="/resources" element={<ResourcesPage />} />
                <Route path="/market" element={<MarketPage />} />
                <Route path="/analytics" element={<AnalyticsPage />} />
                <Route
                  path="/stats"
                  element={
                    settings.stats_tab_enabled ? (
                      <StatsPage />
                    ) : (
                      <Navigate to="/world" replace />
                    )
                  }
                />
                <Route path="/settings" element={<SettingsPage />} />
                <Route path="/about" element={<AboutPage />} />
                <Route path="*" element={<Navigate to="/world" replace />} />
              </Routes>
            </Suspense>
          </div>
        </main>
      </div>
      <MarketPanel />
      <UpdateDialog />
    </div>
  );
}

export default function App() {
  return (
    <Routes>
      <Route path="/overlay/*" element={<OverlayWindow />} />
      <Route path="*" element={<MainWindow />} />
    </Routes>
  );
}
