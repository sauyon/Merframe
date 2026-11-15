import { useCallback, useRef } from "react";
import { toast } from "sonner";
import { errorMessage } from "@/lib/bridge";

export interface Outcome<T> {
  loaded: (value: T) => void;
  failed: (message: string) => void;
  settled?: () => void;
  quiet?: boolean;
}

export function useLatestAttempt() {
  const newest = useRef(0);

  const cancel = useCallback(() => {
    newest.current += 1;
  }, []);

  const attempt = useCallback(
    async <T>(load: () => Promise<T>, outcome: Outcome<T>) => {
      newest.current += 1;
      const current = newest.current;
      const stale = () => current !== newest.current;

      try {
        const value = await load();
        if (!stale()) {
          outcome.loaded(value);
        }
      } catch (error) {
        if (stale()) {
          return;
        }
        const message = errorMessage(error);
        outcome.failed(message);
        if (!outcome.quiet) {
          toast.error(message);
        }
      } finally {
        if (!stale()) {
          outcome.settled?.();
        }
      }
    },
    [],
  );

  return { attempt, cancel };
}
