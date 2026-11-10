import {
  type Column,
  type ColumnDef,
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getSortedRowModel,
  type SortingState,
  useReactTable,
  type VisibilityState,
} from "@tanstack/react-table";
import {
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  ChevronDown,
  Columns3,
} from "lucide-react";
import { Fragment, type ReactNode, useMemo, useState } from "react";
import { Pagination } from "@/components/pagination";
import { SearchInput } from "@/components/search-input";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { usePagePosition } from "@/hooks/use-paged";
import { num } from "@/lib/format";
import { cn } from "@/lib/utils";
import { usePreferencesStore } from "@/stores/preferences-store";

declare module "@tanstack/react-table" {
  interface ColumnMeta<TData, TValue> {
    wrap?: boolean;
    numeric?: boolean;
    label?: string;
  }
}

interface DataTableProps<TData, TValue> {
  tableId: string;
  columns: ColumnDef<TData, TValue>[];
  data: TData[];
  searchPlaceholder?: string;
  searchValue?: (row: TData) => string;
  initialSearch?: string;
  initialSorting?: SortingState;
  primarySort?: SortingState;
  toolbar?: ReactNode;
  filters?: ReactNode;
  emptyMessage?: string;
  renderSubRow?: (row: TData) => ReactNode;
  onRowClick?: (row: TData) => void;
  rowKey?: (row: TData, index: number) => string;
  fixedPageSize?: number;
  initialPageSize?: number;
  resetKey?: string;
}

function columnLabel<TData, TValue>(column: Column<TData, TValue>): string {
  const { header, meta } = column.columnDef;
  if (meta?.label) {
    return meta.label;
  }
  return typeof header === "string" ? header : column.id;
}

export function DataTable<TData, TValue>({
  tableId,
  columns,
  data,
  searchPlaceholder = "Filter",
  searchValue,
  initialSearch = "",
  initialSorting = [],
  primarySort,
  toolbar,
  filters,
  emptyMessage = "Nothing here yet.",
  renderSubRow,
  onRowClick,
  rowKey,
  fixedPageSize,
  initialPageSize,
  resetKey = "",
}: DataTableProps<TData, TValue>) {
  const [sorting, setSorting] = useState<SortingState>(initialSorting);
  const [globalFilter, setGlobalFilter] = useState(initialSearch);
  const [expanded, setExpanded] = useState<string | null>(null);
  const storedPageSize = usePreferencesStore((state) => state.pageSize);
  const hiddenColumns = usePreferencesStore((state) => state.hiddenColumns);
  const setHiddenColumns = usePreferencesStore(
    (state) => state.setHiddenColumns,
  );
  const [ownPageSize, setOwnPageSize] = useState(initialPageSize);
  const pageSize = fixedPageSize ?? ownPageSize ?? storedPageSize;
  const hidden = hiddenColumns[tableId];
  const columnVisibility = useMemo<VisibilityState>(
    () => Object.fromEntries((hidden ?? []).map((id) => [id, false])),
    [hidden],
  );

  const isColumnShown = (entry: { id: string }) =>
    columnVisibility[entry.id] !== false;
  const chosenSorting = sorting.every(isColumnShown) ? sorting : initialSorting;

  const table = useReactTable({
    data,
    columns,
    state: {
      sorting: primarySort ? [...primarySort, ...chosenSorting] : chosenSorting,
      globalFilter,
      columnVisibility,
    },
    globalFilterFn: searchValue
      ? (row, _columnId, filter: string) =>
          searchValue(row.original)
            .toLowerCase()
            .includes(filter.trim().toLowerCase())
      : "auto",
    onSortingChange: setSorting,
    onColumnVisibilityChange: (updater) => {
      const next =
        typeof updater === "function" ? updater(columnVisibility) : updater;
      setHiddenColumns(
        tableId,
        Object.keys(next).filter((id) => next[id] === false),
      );
    },
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
  });

  const shownRows = table.getRowModel().rows;
  const { page, pageCount, setPage } = usePagePosition(
    JSON.stringify([resetKey, sorting, globalFilter]),
    shownRows.length,
    pageSize,
  );
  const rows = shownRows.slice(page * pageSize, (page + 1) * pageSize);
  const span = table.getVisibleLeafColumns().length;
  const hideable = table
    .getAllLeafColumns()
    .filter((column) => column.getCanHide());

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <SearchInput
          value={globalFilter}
          onValueChange={setGlobalFilter}
          placeholder={searchPlaceholder}
        />
        {toolbar}
        <span className="text-muted-foreground ml-auto text-xs">
          {num(shownRows.length)} rows
        </span>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm">
              <Columns3 className="size-4" />
              Columns
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuLabel className="text-xs">
              Shown columns
            </DropdownMenuLabel>
            <DropdownMenuSeparator />
            {hideable.map((column) => (
              <DropdownMenuCheckboxItem
                key={column.id}
                checked={column.getIsVisible()}
                onCheckedChange={(checked) =>
                  column.toggleVisibility(checked === true)
                }
                onSelect={(e) => e.preventDefault()}
              >
                {columnLabel(column)}
              </DropdownMenuCheckboxItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      {filters}

      <div className="bg-card overflow-hidden rounded-xl border">
        <Table>
          <TableHeader>
            {table.getHeaderGroups().map((group) => (
              <TableRow key={group.id} className="hover:bg-transparent">
                {group.headers.map((header) => {
                  const sortable = header.column.getCanSort();
                  const direction = header.column.getIsSorted();
                  return (
                    <TableHead
                      key={header.id}
                      className={cn(
                        header.column.columnDef.meta?.numeric && "text-right",
                      )}
                    >
                      {header.isPlaceholder ? null : sortable ? (
                        <button
                          type="button"
                          onClick={header.column.getToggleSortingHandler()}
                          className="hover:text-foreground inline-flex cursor-pointer items-center gap-1"
                        >
                          {flexRender(
                            header.column.columnDef.header,
                            header.getContext(),
                          )}
                          {direction === "asc" ? (
                            <ArrowUp className="size-3" />
                          ) : direction === "desc" ? (
                            <ArrowDown className="size-3" />
                          ) : (
                            <ArrowUpDown className="size-3 opacity-40" />
                          )}
                        </button>
                      ) : (
                        flexRender(
                          header.column.columnDef.header,
                          header.getContext(),
                        )
                      )}
                    </TableHead>
                  );
                })}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {rows.length === 0 ? (
              <TableRow className="hover:bg-transparent">
                <TableCell
                  colSpan={span}
                  className="text-muted-foreground h-24 text-center"
                >
                  {emptyMessage}
                </TableCell>
              </TableRow>
            ) : (
              rows.map((row, index) => {
                const key = rowKey ? rowKey(row.original, index) : row.id;
                const isOpen = expanded === key;
                return (
                  <Fragment key={key}>
                    <TableRow
                      onClick={
                        onRowClick
                          ? () => onRowClick(row.original)
                          : renderSubRow
                            ? () => setExpanded(isOpen ? null : key)
                            : undefined
                      }
                      className={cn(
                        (renderSubRow || onRowClick) && "cursor-pointer",
                        isOpen && "bg-secondary/50",
                      )}
                    >
                      {row.getVisibleCells().map((cell, cellIndex) => {
                        const meta = cell.column.columnDef.meta;
                        return (
                          <TableCell key={cell.id}>
                            <span
                              className={cn(
                                "flex items-center gap-2",
                                meta?.wrap
                                  ? "max-w-56 min-w-40 whitespace-normal"
                                  : "min-w-max",
                                meta?.numeric &&
                                  "justify-end text-right tabular-nums",
                              )}
                            >
                              {renderSubRow && cellIndex === 0 && (
                                <ChevronDown
                                  className={cn(
                                    "text-muted-foreground size-3.5 shrink-0 transition-transform",
                                    !isOpen && "-rotate-90",
                                  )}
                                />
                              )}
                              {flexRender(
                                cell.column.columnDef.cell,
                                cell.getContext(),
                              )}
                            </span>
                          </TableCell>
                        );
                      })}
                    </TableRow>
                    {renderSubRow && isOpen && (
                      <TableRow className="hover:bg-transparent">
                        <TableCell
                          colSpan={span}
                          className="bg-background/40 whitespace-normal"
                        >
                          {renderSubRow(row.original)}
                        </TableCell>
                      </TableRow>
                    )}
                  </Fragment>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>

      <Pagination
        page={page}
        pageCount={pageCount}
        total={shownRows.length}
        fixedPageSize={fixedPageSize}
        pageSize={ownPageSize}
        onPageSizeChange={
          initialPageSize === undefined ? undefined : setOwnPageSize
        }
        onPageChange={setPage}
      />
    </div>
  );
}
