import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

export interface CheckboxMenuItem {
  key: string;
  label: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}

export function CheckboxMenu({
  trigger,
  heading,
  items,
  align = "start",
  disabled,
  className,
}: {
  trigger: ReactNode;
  heading?: string;
  items: readonly CheckboxMenuItem[];
  align?: "start" | "end";
  disabled?: boolean;
  className?: string;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          className={className}
          disabled={disabled}
        >
          {trigger}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align={align}>
        {heading && (
          <>
            <DropdownMenuLabel className="text-xs">{heading}</DropdownMenuLabel>
            <DropdownMenuSeparator />
          </>
        )}
        {items.map((item) => (
          <DropdownMenuCheckboxItem
            key={item.key}
            checked={item.checked}
            onCheckedChange={(checked) =>
              item.onCheckedChange(checked === true)
            }
            onSelect={(event) => event.preventDefault()}
          >
            {item.label}
          </DropdownMenuCheckboxItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
