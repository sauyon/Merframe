export type YesNo = "yes" | "no";

export const ANY = "any";

export const NO_FILTER_MATCHES = "Nothing matches these filters.";

export function failsYesNo(mode: YesNo, flag: boolean | null): boolean {
  return (mode === "yes") !== flag;
}

export function passesYesNo<K extends string>(
  filters: Partial<Record<K, YesNo>>,
  has: (key: K) => boolean | null,
): boolean {
  for (const [key, mode] of Object.entries(filters) as [K, YesNo][]) {
    if (failsYesNo(mode, has(key))) {
      return false;
    }
  }

  return true;
}

export function withYesNo<K extends string>(
  filters: Partial<Record<K, YesNo>>,
  key: K,
  mode: YesNo | null,
): Partial<Record<K, YesNo>> {
  const next = { ...filters };
  if (mode === null) {
    delete next[key];
  } else {
    next[key] = mode;
  }

  return next;
}

export function toggled<T>(
  list: readonly T[],
  value: T,
  on: boolean = !list.includes(value),
): T[] {
  return on
    ? [...list.filter((kept) => kept !== value), value]
    : list.filter((kept) => kept !== value);
}

export interface FilterOption<T extends string = string> {
  value: T;
  label: string;
}

export function yesNoOptions(yes: string, no: string): FilterOption<YesNo>[] {
  return [
    { value: "yes", label: yes },
    { value: "no", label: no },
  ];
}

export function plainOptions<T extends string>(
  values: readonly T[],
): FilterOption<T>[] {
  return values.map((value) => ({ value, label: value }));
}
