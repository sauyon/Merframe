import { api } from "@/lib/bridge";
import { useAppStore } from "@/stores/app-store";
import { useMarketListingsStore } from "@/stores/market-listings-store";

export async function signOutOfMarket(signedOut?: () => void) {
  await api.marketLogout();
  signedOut?.();
  useMarketListingsStore.getState().clear();
  useAppStore.getState().setStatus(await api.gameStatus());
}
