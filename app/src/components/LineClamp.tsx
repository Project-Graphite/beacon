import { useContext, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { PanelFit } from '../hooks/usePanelHeight.ts';
import { clampLines } from '../lib/lines.ts';

export function LineClamp<T>({ lines, children }: { lines: T[]; children: (shown: T[]) => ReactNode }) {
  const fit = useContext(PanelFit);
  const block = useRef<HTMLDivElement>(null);
  const [expanded, setExpanded] = useState(false);
  const [clip, setClip] = useState<number | null>(null);
  const shown = clampLines(lines, expanded);

  useLayoutEffect(() => {
    const node = block.current;
    if (clip === null || !node) return;
    let current = true;
    void fit(node.scrollHeight - clip).then(() => current && setClip(null));
    return () => {
      current = false;
    };
  }, [clip, fit]);

  return (
    <div ref={block} className="flow-root" style={clip === null ? undefined : { maxHeight: clip, overflow: 'hidden' }}>
      {children(shown)}
      {shown.length < lines.length && (
        <button
          type="button"
          className="press mt-1 rounded-field text-micro text-muted hover:text-ink"
          onClick={() => {
            setClip(block.current?.getBoundingClientRect().height ?? 0);
            setExpanded(true);
          }}
        >
          Show all ({lines.length} lines)
        </button>
      )}
    </div>
  );
}
