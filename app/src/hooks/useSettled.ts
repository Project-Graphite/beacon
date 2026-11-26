import { useCallback, useLayoutEffect, useRef } from 'react';

const SETTLE_MS = 400;

export const settledSince = (changedAt: number) => performance.now() - changedAt >= SETTLE_MS;

export function useSettled(key: unknown) {
  const changedAt = useRef(0);

  useLayoutEffect(() => {
    changedAt.current = performance.now();
  }, [key]);

  return useCallback(() => settledSince(changedAt.current), []);
}
