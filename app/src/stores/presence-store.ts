import { create } from "zustand";
import type { MarketPresence } from "@/types";

interface PresenceState {
  presence: MarketPresence;
  setPresence: (presence: MarketPresence) => void;
}

export const usePresenceStore = create<PresenceState>((set) => ({
  presence: { choice: "untouched", live: null },
  setPresence: (presence) => set({ presence }),
}));
