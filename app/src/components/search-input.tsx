import { Search, X } from "lucide-react";
import type { ComponentProps } from "react";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

export function SearchInput({
  value,
  onValueChange,
  onKeyDown,
  className,
  ...props
}: Omit<ComponentProps<typeof Input>, "value" | "onChange"> & {
  value: string;
  onValueChange: (value: string) => void;
}) {
  return (
    <div className={cn("relative w-64", className)}>
      <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2" />
      <Input
        {...props}
        value={value}
        onChange={(e) => onValueChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape" && value) {
            onValueChange("");
          }
          onKeyDown?.(e);
        }}
        className="pr-9 pl-9"
      />
      {value && (
        <button
          type="button"
          aria-label="Clear"
          title="Clear"
          onMouseDown={(e) => e.preventDefault()}
          onKeyDown={(e) => e.stopPropagation()}
          onClick={() => onValueChange("")}
          className="text-muted-foreground hover:text-foreground absolute top-1/2 right-2 -translate-y-1/2 cursor-pointer rounded-sm p-1"
        >
          <X className="size-4" />
        </button>
      )}
    </div>
  );
}
