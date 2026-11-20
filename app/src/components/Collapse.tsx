import type { ReactNode } from 'react';

interface CollapseProps {
  open: boolean;
  appear?: boolean;
  children: ReactNode;
}

export function Collapse({ open, appear, children }: CollapseProps) {
  return (
    <div
      inert={!open}
      className={`grid transition-[grid-template-rows] duration-200 ease-settle ${open ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'} ${appear ? 'starting:grid-rows-[0fr]' : ''}`}
    >
      <div className="min-h-0 overflow-hidden">
        <div
          className={`transition-[opacity,translate] duration-200 ease-settle ${open ? '' : '-translate-y-1 opacity-0'} ${appear ? 'starting:-translate-y-1 starting:opacity-0' : ''}`}
        >
          {children}
        </div>
      </div>
    </div>
  );
}
