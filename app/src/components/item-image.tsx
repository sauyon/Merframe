import { convertFileSrc } from "@tauri-apps/api/core";
import { ImageOff } from "lucide-react";
import { memo, useEffect, useState } from "react";
import {
  ARCANE_BACKDROP,
  type ArcaneRarity,
  GameIcon,
} from "@/components/game-icon";
import { api, logError } from "@/lib/bridge";
import { cn } from "@/lib/utils";

type Resolved = { src: string } | { failed: true };

const resolved = new Map<string, Resolved>();
const pending = new Map<string, Promise<Resolved>>();

function isGameImage(name: string) {
  return name.startsWith("game/");
}

async function resolve(imageName: string): Promise<Resolved> {
  let state: Resolved;
  try {
    state = isGameImage(imageName)
      ? { src: `/${imageName}` }
      : { src: convertFileSrc(await api.itemImage(imageName)) };
  } catch (error) {
    logError(`Loading image ${imageName}`, error);
    state = { failed: true };
  }
  resolved.set(imageName, state);
  pending.delete(imageName);
  return state;
}

async function load(imageName: string): Promise<Resolved> {
  const cached = resolved.get(imageName);
  if (cached) {
    return cached;
  }
  const inFlight = pending.get(imageName);
  if (inFlight) {
    return inFlight;
  }
  const request = resolve(imageName);
  pending.set(imageName, request);
  return request;
}

export async function prefetchImages(names: (string | null | undefined)[]) {
  const wanted = [
    ...new Set(
      names.filter(
        (name): name is string =>
          typeof name === "string" &&
          name.length > 0 &&
          !isGameImage(name) &&
          !resolved.has(name),
      ),
    ),
  ];
  if (wanted.length === 0) {
    return;
  }

  try {
    const held = await api.prefetchImages(wanted);
    for (const { name, path } of held) {
      resolved.set(name, { src: convertFileSrc(path) });
    }
  } catch (error) {
    logError("Prefetching images", error);
  }
}

/** The loaded source for an item image, or `null` while loading or when it fails. */
export function useItemImageSrc(imageName: string | null) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (!imageName) {
      setSrc(null);
      return;
    }
    let active = true;
    load(imageName).then((state) => {
      if (active) {
        setSrc("src" in state ? state.src : null);
      }
    });
    return () => {
      active = false;
    };
  }, [imageName]);
  return src;
}

interface ItemImageProps {
  imageName?: string | null;
  size?: number;
  alt?: string;
  className?: string;
}

function ItemImageInner({
  imageName,
  size = 32,
  alt = "",
  className,
}: ItemImageProps) {
  const [state, setState] = useState<Resolved | null>(() => {
    if (!imageName) {
      return { failed: true };
    }
    return resolved.get(imageName) ?? null;
  });

  useEffect(() => {
    if (!imageName) {
      setState({ failed: true });
      return;
    }
    const known = resolved.get(imageName);
    if (known) {
      setState(known);
      return;
    }

    let active = true;
    setState(null);
    async function show(name: string) {
      const next = await load(name);
      if (!active) {
        return;
      }
      setState(next);
    }
    show(imageName);

    return () => {
      active = false;
    };
  }, [imageName]);

  const box = cn(
    "bg-muted flex shrink-0 items-center justify-center overflow-hidden rounded-md",
    className,
  );
  const style = { width: size, height: size };

  if (state && "src" in state) {
    return (
      <img
        src={state.src}
        alt={alt}
        width={size}
        height={size}
        loading="lazy"
        className={cn(box, "object-contain")}
        style={style}
        onError={() => setState({ failed: true })}
      />
    );
  }

  return (
    <span
      className={box}
      style={style}
      aria-hidden={alt === ""}
      role="img"
      aria-label={alt || undefined}
    >
      {state && (
        <ImageOff
          className="text-muted-foreground"
          style={{ width: size * 0.5, height: size * 0.5 }}
        />
      )}
    </span>
  );
}

export const ItemImage = memo(ItemImageInner);

const GLYPH_ON_BACKDROP = 0.68;

function ArcaneImageInner({
  rarity,
  size = 32,
  className,
  ...glyph
}: ItemImageProps & { rarity?: ArcaneRarity | null }) {
  if (!rarity) {
    return <ItemImage size={size} className={className} {...glyph} />;
  }
  return (
    <span
      className={cn(
        "relative flex shrink-0 items-center justify-center overflow-hidden rounded-md",
        className,
      )}
      style={{ width: size, height: size }}
    >
      <GameIcon
        name={ARCANE_BACKDROP[rarity]}
        size={size}
        className="absolute inset-0"
      />
      <ItemImage
        {...glyph}
        size={Math.round(size * GLYPH_ON_BACKDROP)}
        className="relative bg-transparent"
      />
    </span>
  );
}

export const ArcaneImage = memo(ArcaneImageInner);
