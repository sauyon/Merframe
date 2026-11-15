import type { ReactNode } from "react";
import { CardGrid, Surface } from "@/components/page";

export function ResetChecklistTileGrid({ children }: { children: ReactNode }) {
  return (
    <CardGrid className="@3xl:grid-cols-3 @5xl:grid-cols-4 @7xl:grid-cols-5">
      {children}
    </CardGrid>
  );
}

export function ResetChecklistTile({ children }: { children: ReactNode }) {
  return <Surface className="gap-1.5 p-3 text-sm">{children}</Surface>;
}
