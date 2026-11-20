import type { ReactNode } from 'react';
import { Collapse } from './Collapse';
import { Icon } from './Icon';

interface FoldedGroupProps {
  motionId: string;
  label: string;
  open: boolean;
  onToggle: () => void;
  children: ReactNode;
}

export function FoldedGroup({ motionId, label, open, onToggle, children }: FoldedGroupProps) {
  return (
    <li data-motion={motionId}>
      <button
        type="button"
        aria-expanded={open}
        onClick={onToggle}
        className="flex h-6 w-full items-center gap-1.5 px-2.5 text-micro text-faint transition-colors duration-120 ease-settle -outline-offset-2 hover:bg-surface hover:text-muted"
      >
        <Icon name="chevron" className={`size-3 transition-transform duration-200 ease-settle ${open ? 'rotate-90' : ''}`} />
        {label}
      </button>
      <Collapse open={open}>
        <ul className="session-list border-t border-line-soft">{children}</ul>
      </Collapse>
    </li>
  );
}
