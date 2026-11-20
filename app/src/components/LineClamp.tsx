import { useState, type ReactNode } from 'react';
import { clampLines } from '../lib/lines.ts';

export function LineClamp<T>({ lines, children }: { lines: T[]; children: (shown: T[]) => ReactNode }) {
  const [expanded, setExpanded] = useState(false);
  const shown = clampLines(lines, expanded);
  return (
    <>
      {children(shown)}
      {shown.length < lines.length && (
        <button
          type="button"
          className="press mt-1 rounded-field text-micro text-muted hover:text-ink"
          onClick={() => setExpanded(true)}
        >
          Show all ({lines.length} lines)
        </button>
      )}
    </>
  );
}
