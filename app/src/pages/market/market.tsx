import { LogOut, Mail, Store } from "lucide-react";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useState,
} from "react";
import { useShallow } from "zustand/react/shallow";
import { Amount } from "@/components/currency";
import { Page, Section, Stat } from "@/components/page";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useLatestAttempt } from "@/hooks/use-latest-attempt";
import { useListen } from "@/hooks/use-listen";
import { useTabParam } from "@/hooks/use-tab-param";
import { api, logError, openExternal, reportError } from "@/lib/bridge";
import { ago, MARKET_CHATS_URL, num } from "@/lib/format";
import { signOutOfMarket } from "@/lib/market-session";
import { notify } from "@/lib/toast";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/app-store";
import { useMarketListingsStore } from "@/stores/market-listings-store";
import { useMarketPanelStore } from "@/stores/market-panel-store";
import { AuctionsTable } from "./auctions";
import { LoginCard } from "./login-card";
import { OrdersTable } from "./orders";
import { LiveStatusBadge, PresenceControl } from "./presence";

type MarketTab = "orders" | "auctions";

const ACTIVE_POLL_MS = 60_000;

export function MarketPage() {
  const status = useAppStore((state) => state.status);
  const setStatus = useAppStore((state) => state.setStatus);
  const { orders, auctions, refreshedAt, setListings } = useMarketListingsStore(
    useShallow((state) => ({
      orders: state.orders,
      auctions: state.auctions,
      refreshedAt: state.at,
      setListings: state.setListings,
    })),
  );
  const openListing = useMarketPanelStore((state) => state.openListing);
  const showPanel = useMarketPanelStore((state) => state.show);
  const [tab, setTab] = useTabParam<MarketTab>("orders");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { attempt, cancel } = useLatestAttempt();
  const [sessionRejected, setSessionRejected] = useState(false);
  const account = status?.market_account ?? null;

  useEffect(() => {
    let last = 0;
    async function markActivity() {
      if (Date.now() - last < 5_000) {
        return;
      }
      last = Date.now();
      try {
        await api.marketActivity();
      } catch (error) {
        logError("Marking market activity", error);
      }
    }
    markActivity();
    document.addEventListener("click", markActivity);
    document.addEventListener("keydown", markActivity);
    return () => {
      document.removeEventListener("click", markActivity);
      document.removeEventListener("keydown", markActivity);
    };
  }, []);

  const reload = useCallback(() => {
    setLoading(true);
    return attempt(
      () => Promise.all([api.marketMyOrders(), api.marketMyAuctions()]),
      {
        loaded: ([nextOrders, nextAuctions]) => {
          setListings(nextOrders, nextAuctions, new Date().toISOString());
          setError(null);
        },
        failed: setError,
        settled: () => setLoading(false),
      },
    );
  }, [attempt, setListings]);

  useEffect(() => {
    const { at } = useMarketListingsStore.getState();
    const fresh = at !== null && Date.now() - Date.parse(at) < ACTIVE_POLL_MS;
    if (account && !fresh) {
      reload();
    } else {
      setLoading(false);
    }
  }, [account, reload]);

  useListen("marketSessionRejected", () => setSessionRejected(true));

  useListen("marketUpdated", () => setError(null));

  const runMarketAction = useCallback(
    async (promise: Promise<unknown>, message: string) => {
      try {
        await promise;
      } catch (error) {
        reportError(error);
        return;
      }
      notify.success(message);
      reload();
    },
    [reload],
  );

  const handleSignedIn = useCallback(async () => {
    setSessionRejected(false);
    try {
      setStatus(await api.gameStatus());
    } catch (error) {
      reportError(error);
    }
  }, [setStatus]);

  const setOrdersVisibility = useCallback(
    (visible: boolean) =>
      runMarketAction(
        api.marketSetVisibility(visible),
        visible ? "All orders visible" : "All orders hidden",
      ),
    [runMarketAction],
  );

  const setAuctionsVisibility = useCallback(
    (visible: boolean) =>
      runMarketAction(
        api.marketSetAuctionsVisibility(visible),
        visible ? "All auctions visible" : "All auctions hidden",
      ),
    [runMarketAction],
  );

  const handleSignOut = useCallback(async () => {
    try {
      await signOutOfMarket(cancel);
    } catch (error) {
      reportError(error);
    }
  }, [cancel]);

  const totals = useMemo(() => {
    const sell = orders.rows.filter((order) => order.order_type === "sell");
    return {
      sell: sell.length,
      buy: orders.rows.length - sell.length,
      plat: sell.reduce(
        (total, order) => total + order.platinum * order.quantity,
        0,
      ),
      missing: orders.rows.filter((order) => order.show_warning).length,
    };
  }, [orders]);

  if (!account) {
    return (
      <Page
        title="warframe.market"
        quote="market"
        actions={
          <Button variant="outline" onClick={showPanel}>
            <Store className="size-4" />
            Buy / Sell
          </Button>
        }
        error={
          sessionRejected ? "warframe.market rejected the stored session" : null
        }
      >
        <LoginCard onDone={handleSignedIn} />
      </Page>
    );
  }

  const openAuctions = auctions.filter((auction) => !auction.closed).length;
  const unread = status?.market_unread ?? 0;
  const tradesLeft = status?.trades_remaining ?? null;

  return (
    <Page
      title="warframe.market"
      description={
        <span className="flex items-center gap-2">
          Signed in as {account.ingame_name}
          <LiveStatusBadge />
        </span>
      }
      actions={
        <>
          <PresenceControl />
          <Button variant="outline" onClick={showPanel}>
            <Store className="size-4" />
            Buy / Sell
          </Button>
          <Button
            variant="outline"
            onClick={() => openExternal(MARKET_CHATS_URL)}
          >
            <Mail className="size-4" />
            Messages
            {unread > 0 && <Badge variant="accent">{num(unread)}</Badge>}
          </Button>
          <Button variant="outline" onClick={handleSignOut}>
            <LogOut className="size-4" />
            Sign Out
          </Button>
        </>
      }
      error={error}
    >
      {tradesLeft !== null && (
        <div className="grid gap-3 @sm:grid-cols-2 @5xl:grid-cols-4">
          <Stat
            label="Trades left today"
            value={
              <span className={cn(tradesLeft === 0 && "text-warning")}>
                {num(tradesLeft)}
              </span>
            }
          />
        </div>
      )}

      <Section
        title="My Listings"
        description={refreshedAt ? `Refreshed ${ago(refreshedAt)}` : undefined}
        action={
          <dl className="flex flex-wrap items-center gap-x-5 gap-y-1 text-sm">
            <Figure label="Sell orders" value={num(totals.sell)} />
            <Figure label="Buy orders" value={num(totals.buy)} />
            <Figure label="Open auctions" value={num(openAuctions)} />
            <Figure
              label="Listed value"
              value={
                <Amount currency="platinum" plain>
                  {num(totals.plat)}
                </Amount>
              }
            />
            <Figure
              label="Missing items"
              value={num(totals.missing)}
              tone={totals.missing > 0 ? "text-warning" : undefined}
              title={
                totals.missing > 0
                  ? "Sell orders above what the account holds"
                  : undefined
              }
            />
          </dl>
        }
      >
        {loading && orders.rows.length === 0 && auctions.length === 0 ? (
          <Skeleton className="h-48 w-full" />
        ) : (
          <div className={cn("transition-opacity", loading && "opacity-60")}>
            <Tabs value={tab} onValueChange={setTab}>
              <TabsList className="mb-4">
                <TabsTrigger value="orders">
                  Orders ({num(orders.rows.length)})
                </TabsTrigger>
                <TabsTrigger value="auctions">
                  Auctions ({num(auctions.length)})
                </TabsTrigger>
              </TabsList>
              <TabsContent value="orders">
                <OrdersTable
                  orders={orders}
                  onCompare={openListing}
                  onRefresh={reload}
                  onSetVisibility={setOrdersVisibility}
                  runMarketAction={runMarketAction}
                />
              </TabsContent>
              <TabsContent value="auctions">
                <AuctionsTable
                  auctions={auctions}
                  onRefresh={reload}
                  onSetVisibility={setAuctionsVisibility}
                  runMarketAction={runMarketAction}
                />
              </TabsContent>
            </Tabs>
          </div>
        )}
      </Section>
    </Page>
  );
}

function Figure({
  label,
  value,
  tone,
  title,
}: {
  label: string;
  value: ReactNode;
  tone?: string;
  title?: string;
}) {
  return (
    <div className="flex items-baseline gap-1.5" title={title}>
      <dt className="text-muted-foreground text-xs">{label}</dt>
      <dd className={cn("font-semibold tabular-nums", tone)}>{value}</dd>
    </div>
  );
}
