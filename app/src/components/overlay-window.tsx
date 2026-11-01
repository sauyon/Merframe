import { type ReactNode, useEffect } from "react";
import { Route, Routes } from "react-router";
import { BuffRing } from "@/components/buff-ring";
import {
  NotificationFade,
  NotificationOverlay,
} from "@/components/notification-overlay";
import {
  DucateringSlot,
  RecommendationSlot,
  RewardSlot,
  RivenSlot,
} from "@/components/overlay-slots";
import { useDisplayPreferences } from "@/hooks/use-display-preferences";
import { useOverlayFeed } from "@/hooks/use-overlay-feed";
import { api, logError } from "@/lib/bridge";
import { cn } from "@/lib/utils";
import { useOverlayStore } from "@/stores/overlay-store";

export const OVERLAY_HASH = "#/overlay/";

const OVERLAY_CLASS = "overlay-window";

function OverlayFrame({
  opacity,
  fit = false,
  scroll = true,
  children,
}: {
  opacity: number;
  fit?: boolean;
  scroll?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      style={{ opacity: opacity / 100 }}
      className={cn(
        "bg-card text-card-foreground flex max-h-full flex-col gap-2 rounded-xl border p-2",
        scroll ? "overflow-y-auto" : "overflow-hidden",
        fit && "mx-auto w-fit",
      )}
    >
      {children}
    </div>
  );
}

export function OverlayWindow() {
  const overlays = useOverlayFeed();
  useDisplayPreferences(overlays);

  useEffect(() => {
    document.documentElement.classList.add(OVERLAY_CLASS);
    return () => document.documentElement.classList.remove(OVERLAY_CLASS);
  }, []);

  const loaded = useOverlayStore((state) => state.loaded);

  useEffect(() => {
    if (!loaded) {
      return;
    }
    const report = async () => {
      try {
        await api.overlayPageReady();
      } catch (error) {
        logError("Reporting the overlay page failed", error);
      }
    };
    report();
  }, [loaded]);

  const comparing = overlays.riven?.before?.pending != null;

  useEffect(() => {
    if (!comparing) {
      return;
    }
    return () => {
      api
        .overlayContentShrank()
        .catch((error) =>
          logError("Reporting the smaller overlay content failed", error),
        );
    };
  }, [comparing]);

  return (
    <div className="h-screen w-full p-2">
      <Routes>
        <Route
          path="relic"
          element={
            overlays.reward && (
              <OverlayFrame opacity={overlays.opacity} fit>
                <RewardSlot trigger={overlays.reward} compact />
              </OverlayFrame>
            )
          }
        />
        <Route
          path="recommend"
          element={
            overlays.recommendation && (
              <OverlayFrame opacity={overlays.opacity} scroll={false}>
                <RecommendationSlot trigger={overlays.recommendation} compact />
              </OverlayFrame>
            )
          }
        />
        <Route
          path="riven"
          element={
            overlays.riven && (
              <OverlayFrame opacity={overlays.opacity} fit>
                <RivenSlot trigger={overlays.riven} compact />
              </OverlayFrame>
            )
          }
        />
        <Route
          path="ducatering"
          element={
            overlays.ducatering && (
              <OverlayFrame opacity={overlays.opacity}>
                <DucateringSlot
                  trigger={overlays.ducatering}
                  clipboard={overlays.ducatering_clipboard}
                  compact
                />
              </OverlayFrame>
            )
          }
        />
        <Route
          path="buffs"
          element={
            overlays.buffs && (
              <BuffRing buffs={overlays.buffs} opacity={overlays.opacity} />
            )
          }
        />
        <Route
          path="notify"
          element={
            overlays.notification && (
              <NotificationFade key={overlays.notification.id}>
                <OverlayFrame opacity={overlays.opacity} scroll={false}>
                  <NotificationOverlay trigger={overlays.notification} />
                </OverlayFrame>
              </NotificationFade>
            )
          }
        />
      </Routes>
    </div>
  );
}
