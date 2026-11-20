import { useState, type CSSProperties } from 'react';
import { rollDirection } from '../lib/motion.ts';

export function RollingNumber({ value }: { value: number }) {
  const [shown, setShown] = useState<{ value: number; previous: number | null }>({ value, previous: null });
  if (shown.value !== value) setShown({ value, previous: shown.value });
  const direction = rollDirection(shown.previous, shown.value);
  return (
    <span
      style={{ '--roll': direction === 'down' ? '-0.7em' : '0.7em' } as CSSProperties}
      className="relative inline-flex overflow-hidden tabular-nums"
    >
      {direction && (
        <span key={`${shown.previous}-${shown.value}`} aria-hidden="true" className="absolute left-0 animate-roll-out">
          {shown.previous}
        </span>
      )}
      <span key={shown.value} className={direction ? 'animate-roll-in' : undefined}>
        {shown.value}
      </span>
    </span>
  );
}
