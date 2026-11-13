import { create } from "zustand";
import type { OverlayState } from "@/types";

const IDLE: OverlayState = {
  seq: 0,
  opacity: 100,
  text_size: "small",
  motion: "system",
  ducatering_clipboard: true,
  reward: null,
  recommendation: null,
  riven: null,
  ducatering: null,
  arbitration: null,
  notification: null,
  buffs: null,
  buffs_style: { warn_seconds: 10, scale: 100, offset: 0 },
  buffs_layout: null,
  vitals: null,
  preview: null,
};

interface OverlayStore {
  overlays: OverlayState;
  loaded: boolean;
  setOverlays: (overlays: OverlayState) => void;
  setLoaded: () => void;
}

export const useOverlayStore = create<OverlayStore>((set) => ({
  overlays: IDLE,
  loaded: false,
  setOverlays: (overlays) => set({ overlays }),
  setLoaded: () => set({ loaded: true }),
}));
