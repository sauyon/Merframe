export function itemAt<T>(items: readonly T[], index: number): T {
  const item = items[index];
  if (item === undefined) {
    throw new RangeError(`No item at index ${index}`);
  }
  return item;
}

export function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = sorted.slice(
    Math.floor((sorted.length - 1) / 2),
    Math.floor(sorted.length / 2) + 1,
  );
  return middle.reduce((sum, value) => sum + value, 0) / middle.length;
}
