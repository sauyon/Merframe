import type { ColumnDef } from "@tanstack/react-table";
import { Tag } from "lucide-react";
import { PolarityIcon } from "@/components/game-icon";
import { GoodRollMarker } from "@/components/good-roll";
import { ItemImage } from "@/components/item-image";
import { SplicedMark, SpliceOptions } from "@/components/spliced-mark";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Hint } from "@/components/ui/hint";
import { num, percent } from "@/lib/format";
import { attributeText, gradeClasses, gradeVariant } from "@/lib/rivens";
import { cn } from "@/lib/utils";
import type { RivenRow } from "@/types";

export const MAX_RIVEN_RANK = 8;

export function rivenColumns(
  onList: (riven: RivenRow) => void,
): ColumnDef<RivenRow>[] {
  return [
    {
      id: "name",
      header: "Riven",
      enableHiding: false,
      accessorFn: (row) => `${row.weapon} ${row.name ?? ""}`.trim(),
      cell: ({ row }) => (
        <span className="flex items-center gap-2">
          <ItemImage
            imageName={row.original.image_name}
            size={32}
            alt={row.original.weapon}
          />
          <span className="flex flex-col">
            <span className="font-medium">{row.original.weapon}</span>
            <Hint as="span">
              {row.original.name ?? row.original.riven_type}
            </Hint>
          </span>
        </span>
      ),
    },
    {
      accessorKey: "grade",
      header: "Roll quality",
      cell: ({ row }) => {
        const letter = row.original.grade_letter;
        return (
          <span className="flex items-center gap-2">
            <Badge variant={gradeVariant(letter)}>{letter}</Badge>
            <Hint as="span">{percent(row.original.grade)}</Hint>
          </span>
        );
      },
    },
    {
      accessorKey: "rank",
      header: "Rank",
      meta: { numeric: true },
      cell: ({ row }) => `${row.original.rank}/${MAX_RIVEN_RANK}`,
    },
    {
      accessorKey: "rerolls",
      header: "Rolls",
      meta: { numeric: true },
      cell: ({ row }) => num(row.original.rerolls),
    },
    {
      accessorKey: "rank_required",
      header: "MR",
      meta: { numeric: true, label: "Mastery rank" },
      cell: ({ row }) => row.original.rank_required ?? "-",
    },
    {
      accessorKey: "polarity",
      header: "Polarity",
      cell: ({ row }) =>
        row.original.polarity ? (
          <PolarityIcon polarity={row.original.polarity} size={20} />
        ) : (
          "-"
        ),
    },
    {
      id: "good_roll",
      header: "Good roll",
      accessorFn: (row) =>
        row.good_roll === null ? -1 : Number(row.good_roll.matches),
      cell: ({ row }) => <GoodRollMarker view={row.original.good_roll} />,
    },
    {
      id: "stats",
      header: "Stats",
      enableSorting: false,
      meta: { wrap: true },
      cell: ({ row }) => (
        <span className="flex flex-col gap-1">
          <span className="flex flex-wrap gap-1">
            {row.original.attributes.map((attribute) => (
              <span key={attribute.tag} className="flex items-center gap-1">
                <Badge variant={attribute.curse ? "warning" : "secondary"}>
                  {attributeText(attribute)}
                </Badge>
                <SplicedMark attribute={attribute} />
                <span
                  className={cn(
                    "font-mono text-xs",
                    gradeClasses(attribute.grade).text,
                  )}
                >
                  {attribute.grade}
                </span>
              </span>
            ))}
          </span>
          <SpliceOptions splices={row.original.splices} />
        </span>
      ),
    },
    {
      id: "list",
      header: "",
      enableSorting: false,
      enableHiding: false,
      cell: ({ row }) =>
        row.original.listed_in_wfm ? (
          <Button
            variant="outline"
            size="sm"
            disabled
            title="Already listed on warframe.market"
          >
            <Tag className="size-3.5" />
            Listed
          </Button>
        ) : (
          <Button
            variant="outline"
            size="sm"
            onClick={(e) => {
              e.stopPropagation();
              onList(row.original);
            }}
          >
            <Tag className="size-3.5" />
            List
          </Button>
        ),
    },
  ];
}
