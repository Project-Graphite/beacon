import { useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { PanelFit } from '../hooks/usePanelHeight.ts';

export function Collapse({ open, children }: { open: boolean; children: ReactNode }) {
  const fit = useContext(PanelFit);
  const inner = useRef<HTMLDivElement>(null);
  const [fitted, setFitted] = useState(open);
  if (!open && fitted) setFitted(false);
  const shown = open && fitted;

  useEffect(() => {
    if (!open || fitted) return;
    let current = true;
    void fit(inner.current?.offsetHeight ?? 0).then(() => current && setFitted(true));
    return () => {
      current = false;
    };
  }, [open, fitted, fit]);

  return (
    <div
      inert={!shown}
      className={`grid transition-[grid-template-rows] duration-200 ease-settle ${shown ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'}`}
    >
      <div className="min-h-0 overflow-hidden">
        <div
          ref={inner}
          className={`transition-[opacity,translate] duration-200 ease-settle ${shown ? '' : '-translate-y-1 opacity-0'}`}
        >
          {children}
        </div>
      </div>
    </div>
  );
}
