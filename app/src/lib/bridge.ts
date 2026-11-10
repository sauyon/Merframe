import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";
import type {
  AppEvent,
  ArbitrationDashboard,
  ArbitrationRun,
  ArbitrationRunExportFormat,
  Auction,
  CommandError,
  ComparedStat,
  CraftDetails,
  DeltaRow,
  FoundryTab,
  GameStatus,
  ImagePath,
  InvasionWeaponPart,
  InventoryTab,
  ItemListings,
  ListingChoices,
  MarketAccount,
  MarketItem,
  MarketMover,
  MarketOrders,
  MarketPresence,
  MarketPresenceChoice,
  MarketWindow,
  MasteryOrdering,
  MasteryTab,
  NewOrder,
  OpeningRow,
  Order,
  OrderPatch,
  OverlayKind,
  OverlayState,
  RelicPlan,
  RelicPlannerTab,
  RelicSource,
  ResourceQuery,
  ResourcesTab,
  RivenComparables,
  RivensTab,
  SeenBuff,
  Settings,
  StatsTab,
  StoredTrade,
  Trade,
  TradeAnalytics,
  WorldStateView,
} from "@/types";

type Args = Record<string, unknown> | undefined;

function call<T>(command: string, args?: Args): Promise<T> {
  return invoke<T>(command, args);
}

export type EventName = AppEvent["event"];

export type EventData<K extends EventName> = K extends EventName
  ? Extract<AppEvent, { event: K }> extends { data: infer D }
    ? D
    : undefined
  : never;

export type Unlisten = () => void;

export function listenTo<K extends EventName>(
  event: K,
  handler: (data: EventData<K>) => void,
): Promise<Unlisten> {
  return listen<AppEvent>("app-event", ({ payload }) => {
    if (payload.event === event) {
      handler(("data" in payload ? payload.data : undefined) as EventData<K>);
    }
  });
}

function commandError(error: unknown): CommandError | null {
  if (error && typeof error === "object" && "message" in error) {
    return error as CommandError;
  }
  return null;
}

export function errorMessage(error: unknown): string {
  const failure = commandError(error);
  if (failure) {
    return failure.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

export function reportError(cause: unknown) {
  toast.error(errorMessage(cause));
}

export function logError(context: string, error: unknown) {
  console.error(`${context}: ${errorMessage(error)}`, error);
}

function since(sinceMs?: number): Args {
  return { sinceMs: sinceMs ?? null };
}

export const api = {
  inventoryTab: () => call<InventoryTab>("inventory_tab"),
  foundryTab: () => call<FoundryTab>("foundry_tab"),
  craftTree: (uniqueName: string) =>
    call<CraftDetails>("craft_tree", { uniqueName }),
  masteryTab: (
    ordering?: MasteryOrdering,
    includeFounders?: boolean | null,
    includeFormaRanks?: boolean,
  ) =>
    call<MasteryTab>("mastery_tab", {
      ordering: ordering ?? null,
      includeFounders: includeFounders ?? null,
      includeFormaRanks: includeFormaRanks ?? null,
    }),
  resourcesTab: (query: ResourceQuery) =>
    call<ResourcesTab>("resources_tab", { query }),
  relicPlannerTab: (squadSize: number, onlyOwned: boolean, wanted: string[]) =>
    call<RelicPlannerTab>("relic_planner_tab", {
      squadSize,
      onlyOwned,
      wanted,
    }),
  relicPlans: (tier: string | null, squadSize: number, wanted: string[]) =>
    call<RelicPlan[]>("relic_plans", { tier, squadSize, wanted }),
  rivensTab: () => call<RivensTab>("rivens_tab"),
  rivenComparables: (weaponSlug: string, shown: ComparedStat[]) =>
    call<RivenComparables | null>("riven_comparables", { weaponSlug, shown }),
  statsTab: (sinceMs?: number) => call<StatsTab>("stats_tab", since(sinceMs)),
  statsDeltas: (sinceMs?: number) =>
    call<DeltaRow[]>("stats_deltas", since(sinceMs)),
  statsOpenings: (sinceMs?: number) =>
    call<OpeningRow[]>("stats_openings", since(sinceMs)),
  statsTrades: (sinceMs?: number) =>
    call<StoredTrade[]>("stats_trades", since(sinceMs)),
  marketMovers: (window: MarketWindow) =>
    call<MarketMover[]>("market_movers", { window }),
  tradeAnalytics: (sinceMs?: number) =>
    call<TradeAnalytics>("trade_analytics", since(sinceMs)),
  recordTrade: (atMs: number, partner: string | null, trade: Trade) =>
    call<number>("record_trade", { atMs, partner, trade }),
  updateTrade: (
    id: number,
    atMs: number,
    partner: string | null,
    trade: Trade,
  ) => call<void>("update_trade", { id, atMs, partner, trade }),
  deleteTrade: (id: number) => call<void>("delete_trade", { id }),
  toggleFavourite: (uniqueName: string) =>
    call<boolean>("toggle_favourite", { uniqueName }),
  relicsFor: (partUniqueName: string) =>
    call<RelicSource[]>("relics_for", { partUniqueName }),
  gameStatus: () => call<GameStatus>("game_status"),
  updatesSupported: () => call<boolean>("updates_supported"),
  overlayState: () => call<OverlayState>("overlay_state"),
  overlayPageReady: () => call<void>("overlay_page_ready"),
  overlayNotify: (title: string, body: string) =>
    call<void>("overlay_notify", { title, body }),
  overlayContentShrank: () => call<void>("overlay_content_shrank"),
  overlayTest: (kind: OverlayKind) => call<void>("overlay_test", { kind }),
  rescanInventory: () => call<GameStatus>("rescan_inventory"),
  exportBundle: () => call<string | null>("export"),
  pickLogFile: () => call<string | null>("pick_log_file"),
  settingsGet: () => call<Settings>("settings_get"),
  settingsSet: (settings: Settings) =>
    call<Settings>("settings_set", { settings }),
  testNotifications: (settings: Settings) =>
    call<void>("test_notifications", { settings }),
  marketLogin: (email: string, password: string) =>
    call<MarketAccount>("market_login", { email, password }),
  marketLogout: () => call<void>("market_logout"),
  marketMyOrders: () => call<MarketOrders>("market_my_orders"),
  marketPostOrder: (order: NewOrder) =>
    call<Order>("market_post_order", { order }),
  marketUpdateOrder: (id: string, patch: OrderPatch) =>
    call<Order>("market_update_order", { id, patch }),
  marketCloseOrder: (id: string, quantity: number) =>
    call<void>("market_close_order", { id, quantity }),
  marketDeleteOrder: (id: string) => call<Order>("market_delete_order", { id }),
  marketActivity: () => call<void>("market_activity"),
  marketPresence: () => call<MarketPresence>("market_presence"),
  marketSetPresence: (choice: MarketPresenceChoice) =>
    call<MarketPresence>("market_set_presence", { choice }),
  marketRemoveAll: () => call<number>("market_remove_all"),
  marketFixOrders: (id?: string) => call<number>("market_fix_orders", { id }),
  marketSetVisibility: (visible: boolean) =>
    call<number>("market_set_visibility", { visible }),
  marketItems: () => call<MarketItem[]>("market_items"),
  marketItemOrders: (slug: string) =>
    call<ItemListings>("market_item_orders", { slug }),
  marketPostRiven: (itemId: string, choices: ListingChoices) =>
    call<string>("market_post_riven", { itemId, choices }),
  marketMyAuctions: () => call<Auction[]>("market_my_auctions"),
  marketEditAuction: (id: string, choices: ListingChoices) =>
    call<Auction>("market_edit_auction", { id, choices }),
  marketSetAuctionVisible: (id: string, visible: boolean) =>
    call<Auction>("market_set_auction_visible", { id, visible }),
  marketCloseAuction: (id: string) =>
    call<void>("market_close_auction", { id }),
  marketSetAuctionsVisibility: (visible: boolean) =>
    call<void>("market_set_auctions_visibility", { visible }),
  worldstate: () => call<WorldStateView>("worldstate"),
  arbitrations: () => call<ArbitrationDashboard>("arbitrations"),
  arbitrationRuns: () => call<ArbitrationRun[]>("arbitration_runs"),
  arbitrationLiveRun: () => call<ArbitrationRun | null>("arbitration_live_run"),
  deleteArbitrationRun: (id: string) =>
    call<void>("delete_arbitration_run", { id }),
  setArbitrationRunVitus: (id: string, vitus: number | null) =>
    call<void>("set_arbitration_run_vitus", { id, vitus }),
  importArbitrationLog: () => call<number | null>("import_arbitration_log"),
  refreshArbitrationRuns: () => call<number>("refresh_arbitration_runs"),
  exportArbitrationRun: (id: string, format: ArbitrationRunExportFormat) =>
    call<string | null>("export_arbitration_run", { id, format }),
  invasionWeaponParts: () =>
    call<InvasionWeaponPart[]>("invasion_weapon_parts"),
  appLogFile: () => call<string>("app_log_file"),
  openDataFolder: () => call<void>("open_data_folder"),
  openGameLogFolder: () => call<void>("open_game_log_folder"),
  itemImage: (imageName: string) => call<string>("item_image", { imageName }),
  seenBuffs: () => call<SeenBuff[]>("seen_buffs"),
  prefetchImages: (names: string[]) =>
    call<ImagePath[]>("prefetch_images", { names }),
};

export async function openExternal(url: string) {
  try {
    await openUrl(url);
  } catch (error) {
    reportError(error);
  }
}
