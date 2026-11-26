import { useEffect, useState } from 'react';
import { useSettled } from './useSettled.ts';

const ARMED_MS = 4000;

export function useArmed(currentId: number | null, destructive: boolean, allow: () => void) {
  const [armed, setArmed] = useState(false);
  const settled = useSettled(armed);

  useEffect(() => {
    if (!armed) return;
    const expire = window.setTimeout(() => setArmed(false), ARMED_MS);
    return () => window.clearTimeout(expire);
  }, [armed]);

  useEffect(() => setArmed(false), [currentId]);

  const press = () => {
    if (destructive && !armed) setArmed(true);
    else if (!destructive || settled()) allow();
  };

  return [armed, press] as const;
}
