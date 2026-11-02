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
  notification: null,
  buffs: null,
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
