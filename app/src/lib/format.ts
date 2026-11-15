const NUMBER = new Intl.NumberFormat("en-US");
const DECIMAL = new Intl.NumberFormat("en-US", {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
});

export function num(value: number | null | undefined): string {
  if (value === null || value === undefined) {
    return "-";
  }
  return NUMBER.format(Math.round(value));
}

export function counted(
  count: number | null | undefined,
  singular: string,
  plural = `${singular}s`,
): string {
  return `${num(count)} ${count === 1 ? singular : plural}`;
}

export function plat(value: number | null | undefined): string {
  if (value === null || value === undefined) {
    return "-";
  }
  return DECIMAL.format(value);
}

export function percent(value: number | null | undefined, digits = 1): string {
  if (value === null || value === undefined) {
    return "-";
  }
  const scale = 10 ** digits;
  const truncated = Math.floor(value * 100 * scale) / scale;
  return `${truncated}%`;
}

export function countdown(seconds: number): string {
  if (seconds <= 0) {
    return "ready";
  }
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = Math.floor(seconds % 60);
  if (days > 0) {
    return `${days}d ${hours}h`;
  }
  if (hours > 0) {
    return `${hours}h ${String(minutes).padStart(2, "0")}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${String(rest).padStart(2, "0")}s`;
  }
  return `${rest}s`;
}

export function secondsUntil(iso: string, now: number = Date.now()): number {
  return Math.round((new Date(iso).getTime() - now) / 1000);
}

export function ago(iso: string | null | undefined): string {
  if (!iso) {
    return "never";
  }
  const seconds = Math.max(
    0,
    Math.round((Date.now() - new Date(iso).getTime()) / 1000),
  );
  if (seconds < 60) {
    return `${seconds}s ago`;
  }
  if (seconds < 3600) {
    return `${Math.floor(seconds / 60)}m ago`;
  }
  if (seconds < 86400) {
    return `${Math.floor(seconds / 3600)}h ago`;
  }
  return `${Math.floor(seconds / 86400)}d ago`;
}

export function dateTime(iso: string): string {
  return new Date(iso).toLocaleString("en-GB", {
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function fullDate(iso: string): string {
  return new Date(iso).toLocaleDateString("en-GB", {
    day: "2-digit",
    month: "short",
    year: "numeric",
  });
}

export function timeOfDay(iso: string): string {
  return new Date(iso).toLocaleTimeString("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function spansYears(days: string[]): boolean {
  return new Set(days.map((day) => day.slice(0, 4))).size > 1;
}

export function dayLabel(day: string, withYear: boolean): string {
  return new Date(`${day}T00:00:00Z`).toLocaleDateString("en-GB", {
    day: "2-digit",
    month: "short",
    ...(withYear ? { year: "numeric" as const } : {}),
    timeZone: "UTC",
  });
}

export function monthLabel(day: string, withYear: boolean): string {
  return new Date(`${day}T00:00:00Z`).toLocaleDateString("en-GB", {
    month: "short",
    ...(withYear ? { year: "numeric" as const } : {}),
    timeZone: "UTC",
  });
}

export function marketUrl(slug: string): string {
  return `https://warframe.market/items/${slug}`;
}

export const MARKET_CHATS_URL = "https://warframe.market/im/chats";
