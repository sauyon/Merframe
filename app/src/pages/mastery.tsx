import { ChevronDown } from "lucide-react";
import { type ReactNode, useCallback, useEffect, useState } from "react";
import { Amount } from "@/components/currency";
import { FavouriteStar } from "@/components/favourite-star";
import { GameIcon } from "@/components/game-icon";
import { ItemImage, prefetchImages } from "@/components/item-image";
import { OptionTabs } from "@/components/option-tabs";
import { CardGrid, EmptyNote, Page, Section, Surface } from "@/components/page";
import { Pagination } from "@/components/pagination";
import { Badge } from "@/components/ui/badge";
import { CheckboxField } from "@/components/ui/checkbox";
import { Hint } from "@/components/ui/hint";
import { Progress } from "@/components/ui/progress";
import { useAsyncData } from "@/hooks/use-async-data";
import { usePaged } from "@/hooks/use-paged";
import { api } from "@/lib/bridge";
import { NO_FILTER_MATCHES } from "@/lib/filters";
import { counted, num, percent } from "@/lib/format";
import { occurrenceKeys } from "@/lib/keys";
import { cn } from "@/lib/utils";
import { useAppStore, useSettings } from "@/stores/app-store";
import type {
  CategoryTotals,
  LevelUpRoute,
  MasteryOrdering,
  Settings,
} from "@/types";

const ORDERINGS: { value: MasteryOrdering; label: string }[] = [
  { value: "closest", label: "Closest to done" },
  { value: "from_relics", label: "Relics you own" },
  { value: "by_platinum", label: "Cheapest to buy" },
];

const INTRINSIC_KINDS = ["duviriIntrinsics", "railjackIntrinsics"];

function foundersNote(founder: boolean, included: boolean): string {
  if (included) {
    return "Excalibur Prime, Skana Prime and Lato Prime count towards mastery.";
  }
  return founder
    ? "The three Founders items are left out."
    : "The three Founders items are left out because this account has no Founder accolade.";
}

function RankBar({
  percent,
  favouritePercent,
  favouriteXp,
}: {
  percent: number;
  favouritePercent: number;
  favouriteXp: number;
}) {
  return (
    <span className="relative mt-1 block">
      <Progress value={percent} />
      {favouritePercent > 0 && (
        <span
          title={`Your favourites are worth ${num(favouriteXp)} XP of this rank`}
          className="bg-accent absolute inset-y-0 block"
          style={{ left: `${percent}%`, width: `${favouritePercent}%` }}
        />
      )}
    </span>
  );
}

function Line({ label, totals }: { label: string; totals?: CategoryTotals }) {
  return (
    <div className="grid grid-cols-[1fr_auto_auto] items-center gap-3 text-sm">
      <span className="text-muted-foreground">{label}</span>
      <span>{percent((totals?.percent ?? 0) / 100)}</span>
      <span className="text-foreground w-20 text-right font-medium">
        {num(totals?.current)}/{num(totals?.max)}
      </span>
    </div>
  );
}

function Tile({
  title,
  progress,
  children,
}: {
  title: string;
  progress: number;
  children: ReactNode;
}) {
  return (
    <Surface className="gap-3">
      <div className="flex items-baseline justify-between">
        <span className="font-semibold">{title}</span>
        <span className="text-primary text-lg font-bold">
          {percent(progress / 100)}
        </span>
      </div>
      <Progress value={progress} />
      <div className="flex flex-col gap-1">{children}</div>
    </Surface>
  );
}

function RouteCard({
  route,
  open,
  onToggle,
}: {
  route: LevelUpRoute;
  open: boolean;
  onToggle: () => void;
}) {
  const memberKeys = occurrenceKeys(
    route.members.map((member) => `${member.name}:${member.detail}`),
  );
  return (
    <div className="bg-card flex flex-col rounded-xl border">
      <button
        type="button"
        onClick={onToggle}
        className="grid cursor-pointer grid-cols-[auto_1fr_auto] items-center gap-3 px-3 py-2 text-left"
      >
        <ChevronDown
          className={cn(
            "text-muted-foreground size-3.5 shrink-0 transition-transform",
            !open && "-rotate-90",
          )}
        />
        <span className="flex min-w-0 flex-col">
          <span className="line-clamp-2 text-sm font-medium">
            {route.label}
          </span>
          <Hint as="span">{counted(route.count, route.unit)}</Hint>
        </span>
        <span className="text-primary text-sm font-bold">
          +{num(route.xp_available)} XP
        </span>
      </button>
      {open && (
        <ul className="max-h-64 overflow-y-auto border-t px-3 py-2">
          {route.members.map((member, index) => (
            <li
              key={memberKeys[index]}
              className="grid grid-cols-[1fr_auto] items-baseline gap-3 py-0.5 text-xs"
            >
              <span className="line-clamp-2 min-w-0">
                {member.name}
                {member.detail && (
                  <span className="text-muted-foreground">
                    {" "}
                    {member.detail}
                  </span>
                )}
              </span>
              <span className="text-muted-foreground tabular-nums">
                +{num(member.xp)} XP
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function MasteryPage() {
  const settings = useSettings();
  const saveSettings = useAppStore((state) => state.saveSettings);
  const [ordering, setOrdering] = useState<MasteryOrdering>("closest");
  const [saveError, setSaveError] = useState<string | null>(null);
  const [openRoute, setOpenRoute] = useState<string | null>(null);

  const includeForma = settings.include_forma_ranks;
  const founders = settings.include_founders_items;

  const load = useCallback(
    () => api.masteryTab(ordering, founders, includeForma),
    [ordering, founders, includeForma],
  );

  const { data, error, loading } = useAsyncData(
    `mastery_tab:${JSON.stringify([ordering, founders, includeForma])}`,
    load,
    ["inventoryUpdated", "pricesUpdated", "favouriteUpdated"],
  );

  const { pageItems: topItems, pagination } = usePaged(
    data?.recommended ?? [],
    JSON.stringify([ordering, founders, includeForma]),
  );
  const routes = data?.routes ?? [];
  const routeTotal = routes.reduce((sum, route) => sum + route.xp_available, 0);

  useEffect(() => {
    prefetchImages(topItems.map((item) => item.image_name));
  }, [topItems]);

  const choose = async (patch: Partial<Settings>) =>
    setSaveError(await saveSettings(patch));

  const founder = data?.founder ?? false;
  const includeFounders = founders ?? data?.include_founders ?? false;

  const foundersToggle = (
    <CheckboxField
      id="includeFounders"
      checked={includeFounders}
      onChange={(value) => choose({ include_founders_items: value })}
    >
      Founders items
    </CheckboxField>
  );

  const summary = data?.summary;
  return (
    <Page
      title="Mastery"
      quote="mastery"
      actions={foundersToggle}
      loading={loading && !data}
      error={error ?? saveError}
    >
      <Surface className="w-fit flex-row items-center gap-6 px-6 py-4">
        <div className="flex items-center gap-3">
          <GameIcon name="mastered" size={40} alt="Mastery rank" />
          <span className="flex flex-col">
            <span className="text-foreground text-4xl leading-none font-bold tabular-nums">
              {num(data?.rank)}
            </span>
          </span>
        </div>
        <div className="flex w-80 flex-col gap-2">
          <div className="flex items-baseline justify-between gap-3">
            <span className="text-foreground text-lg font-semibold tabular-nums">
              {num(data?.rank_xp_earned)} / {num(data?.rank_xp_span)} XP
            </span>
            <span className="text-primary text-lg font-bold tabular-nums">
              {percent((data?.percent ?? 0) / 100, 2)}
            </span>
          </div>
          <RankBar
            percent={data?.percent ?? 0}
            favouritePercent={data?.favourite_percent ?? 0}
            favouriteXp={data?.favourite_xp ?? 0}
          />
        </div>
      </Surface>

      <div className="grid gap-4 @4xl:grid-cols-3">
        <Tile title="Game content" progress={summary?.content_percent ?? 0}>
          <Line label="Warframes / Archwings" totals={summary?.warframes} />
          <Line label="Weapons" totals={summary?.weapons} />
          <Line label="Companions" totals={summary?.companions} />
        </Tile>
        <Tile title="Star chart" progress={summary?.star_percent ?? 0}>
          <Line label="Normal" totals={summary?.star_normal} />
          <Line label="Junctions" totals={summary?.star_junctions} />
          <Line label="Steel Path" totals={summary?.star_steel} />
          <Line
            label="Steel Path junctions"
            totals={summary?.star_steel_junctions}
          />
        </Tile>
        <Tile title="Intrinsics" progress={summary?.intrinsic_percent ?? 0}>
          <Line label="Railjack" totals={summary?.intrinsic_railjack} />
          <Line label="Duviri" totals={summary?.intrinsic_duviri} />
        </Tile>
      </div>

      <Section title="Mastery Left to Earn">
        {routes.length === 0 ? (
          <EmptyNote>Everything is mastered.</EmptyNote>
        ) : (
          <div
            className={cn(
              "grid items-start gap-2 transition-opacity @sm:grid-cols-2 @5xl:grid-cols-3",
              loading && "opacity-60",
            )}
          >
            {routes.map((route) => (
              <RouteCard
                key={route.kind}
                route={route}
                open={openRoute === route.kind}
                onToggle={() =>
                  setOpenRoute(openRoute === route.kind ? null : route.kind)
                }
              />
            ))}
          </div>
        )}
        {routes.length > 0 && (
          <p className="text-muted-foreground mt-3 text-right text-sm">
            Total{" "}
            <span className="text-primary font-bold">{num(routeTotal)}</span> XP
          </p>
        )}
      </Section>

      <Section
        title="Fastest Mastery From Here"
        description={foundersNote(founder, includeFounders)}
        action={
          <div className="flex flex-wrap items-center gap-3">
            <CheckboxField
              id="includeForma"
              checked={includeForma}
              onChange={(value) => choose({ include_forma_ranks: value })}
            >
              Include ranks past 30
            </CheckboxField>
            <OptionTabs
              value={ordering}
              options={ORDERINGS}
              onChange={setOrdering}
            />
          </div>
        }
      >
        <CardGrid className={cn("transition-opacity", loading && "opacity-60")}>
          {topItems.length === 0 ? (
            <EmptyNote>{NO_FILTER_MATCHES}</EmptyNote>
          ) : (
            topItems.map((item) => (
              <div
                key={item.unique_name}
                className="bg-card flex gap-3 rounded-xl border p-3"
              >
                <ItemImage
                  imageName={item.image_name}
                  size={52}
                  alt={item.name}
                />
                <div className="flex min-w-0 flex-1 flex-col gap-1">
                  <span className="flex items-center gap-1">
                    {!INTRINSIC_KINDS.includes(item.kind) && (
                      <FavouriteStar
                        uniqueName={item.unique_name}
                        favourite={item.favourite}
                        size="small"
                      />
                    )}
                    <span className="line-clamp-2 text-sm font-medium">
                      {item.name}
                    </span>
                  </span>
                  <Hint as="span">+{num(item.level.xp_remaining)} XP</Hint>
                  <span className="flex flex-wrap items-center gap-1">
                    {item.owned && item.level.max > 0 && (
                      <Badge variant="secondary">
                        Level {item.level.current}/{item.level.max}
                      </Badge>
                    )}
                    {item.owned && <Badge variant="secondary">Owned</Badge>}
                    {!item.owned && item.acquisition.relic_probability > 0 && (
                      <Badge variant="muted">
                        {percent(item.acquisition.relic_probability, 2)} relics
                      </Badge>
                    )}
                    {!item.owned && (
                      <Badge variant="accent">
                        <Amount currency="platinum" plain size={14}>
                          {num(item.acquisition.plat_cost)}
                        </Amount>
                      </Badge>
                    )}
                  </span>
                  {!item.owned && item.components.length > 0 && (
                    <span className="flex flex-wrap items-center gap-1 pt-1">
                      {item.components.map((component) => (
                        <span
                          key={component.unique_name}
                          title={component.name}
                          className={
                            component.enough ? "" : "opacity-40 grayscale"
                          }
                        >
                          <ItemImage
                            imageName={component.image_name}
                            size={20}
                          />
                        </span>
                      ))}
                    </span>
                  )}
                </div>
              </div>
            ))
          )}
        </CardGrid>
        <Pagination {...pagination} className="mt-3" />
        {ordering === "by_platinum" && (data?.plat_total ?? 0) > 0 && (
          <p className="text-muted-foreground mt-3 text-right text-sm">
            Total{" "}
            <span className="text-primary font-bold">
              {num(data?.plat_total)}
            </span>{" "}
            platinum
          </p>
        )}
      </Section>
    </Page>
  );
}
