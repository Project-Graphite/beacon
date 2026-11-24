import { useEffect, useState } from 'react';

const ARMED_MS = 4000;
const SECOND_PRESS_GAP_MS = 400;

export function useArmed(currentId: number | null) {
  const [armed, setArmed] = useState(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    setReady(false);
    if (!armed) return;
    const settle = window.setTimeout(() => setReady(true), SECOND_PRESS_GAP_MS);
    const expire = window.setTimeout(() => setArmed(false), ARMED_MS);
    return () => {
      window.clearTimeout(settle);
      window.clearTimeout(expire);
    };
  }, [armed]);

  useEffect(() => setArmed(false), [currentId]);

  return [armed, setArmed, ready] as const;
}
