import { Command as CommandPrimitive } from "cmdk";
import type * as React from "react";

import { cn } from "@/lib/utils";

function Command({
  className,
  ...props
}: React.ComponentProps<typeof CommandPrimitive>) {
  return (
    <CommandPrimitive
      data-slot="command"
      className={cn("flex w-full flex-col", className)}
      {...props}
    />
  );
}

function CommandInput(
  props: Omit<React.ComponentProps<typeof CommandPrimitive.Input>, "asChild">,
) {
  return (
    <CommandPrimitive.Input data-slot="command-input" asChild {...props} />
  );
}

function CommandList({
  className,
  ...props
}: React.ComponentProps<typeof CommandPrimitive.List>) {
  return (
    <CommandPrimitive.List
      data-slot="command-list"
      className={cn("max-h-80 overflow-x-hidden overflow-y-auto", className)}
      {...props}
    />
  );
}

function CommandItem({
  className,
  ...props
}: React.ComponentProps<typeof CommandPrimitive.Item>) {
  return (
    <CommandPrimitive.Item
      data-slot="command-item"
      className={cn(
        "data-[selected=true]:bg-secondary flex cursor-pointer items-center gap-2 px-3 py-1.5 text-sm outline-hidden",
        className,
      )}
      {...props}
    />
  );
}

export { Command, CommandInput, CommandItem, CommandList };
