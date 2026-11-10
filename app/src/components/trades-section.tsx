import type { ColumnDef } from "@tanstack/react-table";
import { Ellipsis } from "lucide-react";
import { type ReactNode, useCallback, useMemo, useState } from "react";
import { DataTable } from "@/components/data-table";
import { Section } from "@/components/page";
import { TradeDialog, type TradeEntry } from "@/components/trade-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { api, reportError } from "@/lib/bridge";
import { dateTime, num } from "@/lib/format";
import { notify } from "@/lib/toast";
import type { StoredTrade, TradeItem } from "@/types";

function tradeItemLabel(item: TradeItem): string {
  return item.rank === null ? item.name : `${item.name} R${item.rank}`;
}

function ItemBadges({
  items,
  variant,
}: {
  items: TradeItem[];
  variant: "secondary" | "accent";
}) {
  return (
    <span className="flex flex-wrap gap-1">
      {items.map((item) => (
        <Badge key={tradeItemLabel(item)} variant={variant}>
          {tradeItemLabel(item)} x{item.count}
        </Badge>
      ))}
    </span>
  );
}

type Editing =
  | { kind: "new" }
  | { kind: "edit" | "delete"; trade: StoredTrade };

export function TradesSection({
  trades,
  tableId,
  description,
  filter,
}: {
  trades: StoredTrade[];
  tableId: string;
  description?: string;
  filter?: ReactNode;
}) {
  const [editing, setEditing] = useState<Editing | null>(null);

  const close = useCallback(() => setEditing(null), []);

  const save = useCallback(
    async (entry: TradeEntry) => {
      try {
        if (editing?.kind === "edit") {
          await api.updateTrade(
            editing.trade.id,
            entry.atMs,
            entry.partner,
            entry.trade,
          );
          notify.success("Trade saved");
        } else {
          await api.recordTrade(entry.atMs, entry.partner, entry.trade);
          notify.success("Trade recorded");
        }
      } catch (error) {
        reportError(error);
      }
    },
    [editing],
  );

  const remove = useCallback(async () => {
    if (editing?.kind !== "delete") {
      return;
    }
    try {
      await api.deleteTrade(editing.trade.id);
      notify.success("Trade deleted");
    } catch (error) {
      reportError(error);
    }
    setEditing(null);
  }, [editing]);

  const columns = useMemo<ColumnDef<StoredTrade>[]>(
    () => [
      {
        accessorKey: "at",
        header: "When",
        enableHiding: false,
        cell: ({ row }) => dateTime(row.original.at),
      },
      {
        accessorKey: "partner",
        header: "Partner",
        cell: ({ row }) => row.original.partner ?? "-",
      },
      {
        id: "plat",
        header: "Plat",
        meta: { numeric: true },
        accessorFn: (row) => row.trade.plat,
        cell: ({ row }) => (
          <span className="text-primary font-bold">
            {num(row.original.trade.plat)}
          </span>
        ),
      },
      {
        id: "offered",
        header: "Offered",
        enableSorting: false,
        meta: { wrap: true },
        cell: ({ row }) => (
          <ItemBadges items={row.original.trade.offered} variant="secondary" />
        ),
      },
      {
        id: "received",
        header: "Received",
        enableSorting: false,
        meta: { wrap: true },
        cell: ({ row }) => (
          <ItemBadges items={row.original.trade.received} variant="accent" />
        ),
      },
      {
        id: "actions",
        header: "",
        enableSorting: false,
        enableHiding: false,
        cell: ({ row }) => (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="size-8"
                aria-label="Trade actions"
              >
                <Ellipsis className="size-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem
                onSelect={() =>
                  setEditing({ kind: "edit", trade: row.original })
                }
              >
                Edit trade
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                variant="destructive"
                onSelect={() =>
                  setEditing({ kind: "delete", trade: row.original })
                }
              >
                Delete trade
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        ),
      },
    ],
    [],
  );

  return (
    <Section
      title="Trades"
      description={description}
      action={
        <div className="flex flex-wrap items-center justify-end gap-2">
          {filter}
          <Button
            variant="outline"
            size="sm"
            onClick={() => setEditing({ kind: "new" })}
          >
            Record Trade
          </Button>
        </div>
      }
    >
      <DataTable
        tableId={tableId}
        columns={columns}
        data={trades}
        searchPlaceholder="Filter trades"
        initialSorting={[{ id: "at", desc: true }]}
        rowKey={(row) => String(row.id)}
        emptyMessage="No trades in this window."
      />
      {(editing?.kind === "new" || editing?.kind === "edit") && (
        <TradeDialog
          stored={editing.kind === "edit" ? editing.trade : null}
          onSubmit={save}
          onClose={close}
        />
      )}
      <ConfirmDialog
        open={editing?.kind === "delete"}
        onOpenChange={(open) => !open && close()}
        title="Delete Trade"
        description={
          editing?.kind === "delete" &&
          `The trade from ${dateTime(editing.trade.at)} is removed from the history. This cannot be undone.`
        }
        confirmLabel="Delete"
        destructive
        onConfirm={remove}
      />
    </Section>
  );
}
