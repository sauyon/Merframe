import { ChevronLeft, ChevronRight } from "lucide-react";
import type * as React from "react";
import {
  type ChevronProps,
  DayPicker,
  getDefaultClassNames,
} from "react-day-picker";

import { buttonVariants } from "@/components/ui/button";
import { cn } from "@/lib/utils";

function Chevron({ orientation, className, ...rest }: ChevronProps) {
  const Icon = orientation === "left" ? ChevronLeft : ChevronRight;
  return <Icon className={cn("size-4", className)} {...rest} />;
}

function Calendar({
  className,
  classNames,
  showOutsideDays = true,
  ...props
}: React.ComponentProps<typeof DayPicker>) {
  const defaults = getDefaultClassNames();
  return (
    <DayPicker
      showOutsideDays={showOutsideDays}
      className={cn("p-3", className)}
      classNames={{
        root: cn("w-fit", defaults.root),
        months: cn("relative flex flex-col gap-4", defaults.months),
        month: cn("flex w-full flex-col gap-4", defaults.month),
        nav: cn(
          "absolute inset-x-0 top-0 flex w-full items-center justify-between",
          defaults.nav,
        ),
        button_previous: cn(
          buttonVariants({ variant: "ghost", size: "icon" }),
          "size-8",
          defaults.button_previous,
        ),
        button_next: cn(
          buttonVariants({ variant: "ghost", size: "icon" }),
          "size-8",
          defaults.button_next,
        ),
        month_caption: cn(
          "flex h-8 w-full items-center justify-center px-8",
          defaults.month_caption,
        ),
        dropdowns: cn(
          "flex w-full items-center justify-center gap-1.5 text-sm font-medium",
          defaults.dropdowns,
        ),
        dropdown_root: cn(
          "border-input has-focus:border-ring has-focus:ring-ring/50 relative rounded-md border shadow-xs has-focus:ring-[3px]",
          defaults.dropdown_root,
        ),
        dropdown: cn("absolute inset-0 opacity-0", defaults.dropdown),
        caption_label: cn(
          "flex h-8 items-center gap-1 rounded-md pr-1 pl-2 text-sm select-none [&>svg]:size-3.5",
          defaults.caption_label,
        ),
        weekdays: cn("flex", defaults.weekdays),
        weekday: cn(
          "text-muted-foreground flex-1 rounded-md text-xs font-normal select-none",
          defaults.weekday,
        ),
        week: cn("mt-2 flex w-full", defaults.week),
        day: cn(
          "group/day relative aspect-square size-8 p-0 text-center select-none",
          defaults.day,
        ),
        day_button: cn(
          buttonVariants({ variant: "ghost" }),
          "size-8 p-0 text-sm font-normal group-data-[selected=true]/day:bg-primary group-data-[selected=true]/day:text-primary-foreground",
          defaults.day_button,
        ),
        today: cn("text-primary", defaults.today),
        outside: cn("text-muted-foreground", defaults.outside),
        disabled: cn("text-muted-foreground opacity-50", defaults.disabled),
        hidden: cn("invisible", defaults.hidden),
        ...classNames,
      }}
      components={{ Chevron }}
      {...props}
    />
  );
}

export { Calendar };
