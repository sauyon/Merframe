import {
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
} from "lucide-react";
import { useRef, useState } from "react";
import { flushSync } from "react-dom";
import { Button } from "@/components/ui/button";
import { Hint } from "@/components/ui/hint";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { num } from "@/lib/format";
import { cn } from "@/lib/utils";
import { usePreferencesStore } from "@/stores/preferences-store";

export const TABLE_PAGE_SIZES = [10, 25, 50, 100];
export const GRID_PAGE_SIZES = [12, 24, 48, 96];

export function Pagination({
  page,
  pageCount,
  total,
  fixedPageSize,
  pageSize: ownPageSize,
  pageSizes = TABLE_PAGE_SIZES,
  onPageSizeChange,
  onPageChange,
  className,
}: {
  page: number;
  pageCount: number;
  total: number;
  fixedPageSize?: number;
  pageSize?: number;
  pageSizes?: number[];
  onPageSizeChange?: (pageSize: number) => void;
  onPageChange: (page: number) => void;
  className?: string;
}) {
  const storedPageSize = usePreferencesStore((state) => state.pageSize);
  const setStoredPageSize = usePreferencesStore((state) => state.setPageSize);
  const pageSize = ownPageSize ?? storedPageSize;
  const setPageSize = onPageSizeChange ?? setStoredPageSize;
  const [typed, setTyped] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const changePage = (next: number) => {
    flushSync(() => onPageChange(next));
    root.current?.parentElement?.scrollIntoView({ block: "start" });
  };

  const commitTyped = () => {
    const wanted =
      Math.min(Math.max(Number.parseInt(typed ?? "", 10), 1), pageCount) - 1;
    if (!Number.isNaN(wanted) && wanted !== page) {
      changePage(wanted);
    }

    setTyped(null);
  };

  if (total <= (fixedPageSize ?? Math.min(...pageSizes))) {
    return null;
  }

  return (
    <div
      ref={root}
      className={cn("flex flex-wrap items-center justify-end gap-2", className)}
    >
      {fixedPageSize === undefined && (
        <>
          <Hint as="span">Per page</Hint>
          <Select
            value={String(pageSize)}
            onValueChange={(value) => {
              setPageSize(Number(value));
              changePage(0);
            }}
          >
            <SelectTrigger size="sm" aria-label="Per page" className="text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {pageSizes.map((size) => (
                <SelectItem key={size} value={String(size)} className="text-xs">
                  {size}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </>
      )}
      <Button
        variant="outline"
        size="icon"
        className="ml-2"
        aria-label="First page"
        title="First page"
        onClick={() => changePage(0)}
        disabled={page === 0}
      >
        <ChevronsLeft className="size-4" />
      </Button>
      <Button
        variant="outline"
        size="icon"
        aria-label="Previous page"
        title="Previous page"
        onClick={() => changePage(page - 1)}
        disabled={page === 0}
      >
        <ChevronLeft className="size-4" />
      </Button>
      <Hint as="span">Page</Hint>
      <Input
        inputMode="numeric"
        aria-label="Page"
        className="h-8 w-14 text-center text-xs"
        value={typed ?? String(page + 1)}
        onChange={(e) => setTyped(e.target.value)}
        onFocus={(e) => e.target.select()}
        onBlur={commitTyped}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            commitTyped();
          }
        }}
      />
      <Hint as="span">of {num(pageCount)}</Hint>
      <Button
        variant="outline"
        size="icon"
        aria-label="Next page"
        title="Next page"
        onClick={() => changePage(page + 1)}
        disabled={page >= pageCount - 1}
      >
        <ChevronRight className="size-4" />
      </Button>
      <Button
        variant="outline"
        size="icon"
        aria-label="Last page"
        title="Last page"
        onClick={() => changePage(pageCount - 1)}
        disabled={page >= pageCount - 1}
      >
        <ChevronsRight className="size-4" />
      </Button>
    </div>
  );
}
