import { useEffect, useRef } from "react";

export function useRollover(
  ends: readonly string[],
  now: number,
  onRollover: () => void,
) {
  const latest = useRef(onRollover);
  latest.current = onRollover;

  const rolledOver = useRef(Number.NEGATIVE_INFINITY);
  const lastPassed = Math.max(
    ...ends.map((end) => new Date(end).getTime()).filter((end) => end <= now),
  );
  useEffect(() => {
    if (lastPassed <= rolledOver.current) {
      return;
    }

    rolledOver.current = lastPassed;
    latest.current();
  }, [lastPassed]);
}
