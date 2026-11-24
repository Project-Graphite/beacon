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
      className={`grid transition-[grid-template-rows] ${shown ? 'grid-rows-[1fr] duration-[240ms] ease-settle' : 'grid-rows-[0fr] duration-[280ms] ease-fold'}`}
    >
      <div className="min-h-0 overflow-hidden">
        <div
          ref={inner}
          className={`transition-[opacity,translate] ${shown ? 'duration-[240ms] ease-settle' : '-translate-y-2 opacity-0 delay-[60ms] duration-[200ms] ease-fold'}`}
        >
          {children}
        </div>
      </div>
    </div>
  );
}
