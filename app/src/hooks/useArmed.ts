import { useEffect, useState } from 'react';

const ARMED_MS = 4000;

export function useArmed(currentId: number | null) {
  const [armed, setArmed] = useState(false);

  useEffect(() => {
    if (!armed) return;
    const timer = window.setTimeout(() => setArmed(false), ARMED_MS);
    return () => window.clearTimeout(timer);
  }, [armed]);

  useEffect(() => setArmed(false), [currentId]);

  return [armed, setArmed] as const;
}
