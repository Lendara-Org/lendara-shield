import { useEffect, useRef } from "react";

/**
 * Calls `tick` every `intervalMs` while `active`, one call at a time (the
 * next waits for the previous to settle). Stops on unmount.
 */
export function usePoll(active: boolean, intervalMs: number, tick: () => Promise<void>): void {
  const latest = useRef(tick);
  useEffect(() => {
    latest.current = tick;
  });

  useEffect(() => {
    if (!active) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const loop = async () => {
      try {
        await latest.current();
      } catch {
        // A failed poll is retried on the next tick.
      }
      if (!stopped) timer = setTimeout(() => void loop(), intervalMs);
    };
    timer = setTimeout(() => void loop(), intervalMs);
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [active, intervalMs]);
}
