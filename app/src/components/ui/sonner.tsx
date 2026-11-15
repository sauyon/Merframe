"use client";

import {
  CircleCheckIcon,
  InfoIcon,
  OctagonXIcon,
  TriangleAlertIcon,
} from "lucide-react";
import { useTheme } from "next-themes";
import { Toaster as Sonner, type ToasterProps } from "sonner";
import { Spinner } from "@/components/ui/spinner";
import { useAppStore } from "@/stores/app-store";
import type { ToastPosition } from "@/types";

const POSITIONS: Record<ToastPosition, ToasterProps["position"]> = {
  top_left: "top-left",
  top_centre: "top-center",
  top_right: "top-right",
  bottom_left: "bottom-left",
  bottom_centre: "bottom-center",
  bottom_right: "bottom-right",
};

const Toaster = ({ ...props }: ToasterProps) => {
  const { theme = "system" } = useTheme();
  const settings = useAppStore((state) => state.settings);

  return (
    <Sonner
      theme={theme as ToasterProps["theme"]}
      position={POSITIONS[settings?.toast_position ?? "bottom_right"]}
      className="toaster group"
      icons={{
        success: <CircleCheckIcon className="size-4" />,
        info: <InfoIcon className="size-4" />,
        warning: <TriangleAlertIcon className="size-4" />,
        error: <OctagonXIcon className="size-4" />,
        loading: <Spinner />,
      }}
      style={
        {
          "--normal-bg": "var(--card)",
          "--normal-text": "var(--card-foreground)",
          "--normal-border": "var(--border)",
          "--border-radius": "var(--radius)",
        } as React.CSSProperties
      }
      toastOptions={{
        classNames: {
          toast: "shadow-sm!",
          title: "font-semibold!",
          description: "text-muted-foreground!",
          success: "[&_[data-icon]]:text-primary",
          info: "[&_[data-icon]]:text-accent",
          warning: "[&_[data-icon]]:text-warning",
          error: "border-destructive/50! [&_[data-icon]]:text-destructive",
        },
      }}
      {...props}
    />
  );
};

export { Toaster };
