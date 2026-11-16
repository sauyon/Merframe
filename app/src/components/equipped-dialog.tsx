import { GameIcon } from "@/components/game-icon";
import { ItemImage } from "@/components/item-image";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { counted, num } from "@/lib/format";
import type { ModHolder } from "@/types";

const CONFIG_LETTERS = "ABCDEF";

function orokinUpgrade(holder: ModHolder): string {
  return holder.takes_orokin_reactor ? "Orokin Reactor" : "Orokin Catalyst";
}

export function EquippedDialog({
  name,
  rank,
  holders,
  label,
}: {
  name: string;
  rank: number | null;
  holders: ModHolder[];
  label: string;
}) {
  return (
    <Dialog>
      <DialogTrigger className="text-muted-foreground hover:text-foreground -mt-1 cursor-pointer truncate text-left text-xs transition-colors">
        Equipped in {label}
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{name}</DialogTitle>
          <DialogDescription>
            {rank === null ? "Equipped" : `Rank ${rank}, equipped`} in{" "}
            {counted(holders.length, "item")}
          </DialogDescription>
        </DialogHeader>
        <ul className="-mr-2 flex max-h-96 flex-col gap-1 overflow-y-auto pr-2">
          {holders.map((holder) => (
            <li
              key={holder.item_id}
              className="flex items-center gap-3 rounded-md border p-2"
            >
              <ItemImage imageName={holder.image_name} size={40} />
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="flex items-baseline gap-2">
                  <span className="truncate text-sm font-medium">
                    {holder.custom_name ?? holder.name}
                  </span>
                  {holder.custom_name && (
                    <span className="text-muted-foreground truncate text-xs">
                      {holder.name}
                    </span>
                  )}
                </span>
                <span className="text-muted-foreground flex flex-wrap items-center gap-x-2 text-xs tabular-nums">
                  {holder.rank !== null && <span>Rank {holder.rank}</span>}
                  <span
                    title={`${num(holder.forma)} Forma installed`}
                    className="flex items-center gap-1"
                  >
                    <GameIcon name="forma" size={16} alt="Forma" />
                    {holder.forma}
                  </span>
                  {holder.archon_shards > 0 && (
                    <span
                      title={`${counted(holder.archon_shards, "Archon Shard")} installed`}
                      className="flex items-center gap-1"
                    >
                      <GameIcon
                        name="archon-shard"
                        size={16}
                        alt="Archon Shards"
                      />
                      {holder.archon_shards}
                    </span>
                  )}
                  {holder.orokin_upgrade && (
                    <GameIcon
                      name={
                        holder.takes_orokin_reactor
                          ? "orokin-reactor"
                          : "orokin-catalyst"
                      }
                      size={16}
                      alt={orokinUpgrade(holder)}
                      title={orokinUpgrade(holder)}
                    />
                  )}
                  {holder.exilus_adapter && (
                    <GameIcon
                      name={
                        holder.takes_orokin_reactor
                          ? "exilus-adapter"
                          : "exilus-weapon-adapter"
                      }
                      size={16}
                      alt="Exilus Adapter"
                      title="Exilus Adapter"
                    />
                  )}
                </span>
                <span className="mt-1 flex flex-wrap gap-1">
                  {holder.configs.map((config) => (
                    <Badge key={config} variant="muted">
                      Config {CONFIG_LETTERS[config]}
                    </Badge>
                  ))}
                </span>
              </span>
            </li>
          ))}
        </ul>
      </DialogContent>
    </Dialog>
  );
}
