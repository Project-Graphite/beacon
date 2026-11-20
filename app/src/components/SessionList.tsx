import type { ReactNode } from 'react';
import type { Session } from '../beacon';
import type { SessionGroup } from '../lib/sessions.ts';
import { Icon } from './Icon';

interface SessionListProps {
  groups: SessionGroup[];
  ended: Session[];
  endedOpen: boolean;
  onToggleEnded: () => void;
  renderCard: (session: Session) => ReactNode;
}

export function SessionList({ groups, ended, endedOpen, onToggleEnded, renderCard }: SessionListProps) {
  return (
    <ul aria-label="Sessions" className="session-list">
      {groups.flatMap((group) => [
        groups.length > 1 && (
          <li key={`group-${group.label}`} data-group data-motion={`group:${group.label}`} className="flex h-5 items-center px-2.5">
            <h2 className="text-micro text-faint">{group.label}</h2>
          </li>
        ),
        ...group.sessions.map(renderCard),
      ])}
      {ended.length > 0 && (
        <li key="ended" data-motion="ended">
          <button
            type="button"
            aria-expanded={endedOpen}
            onClick={onToggleEnded}
            className="flex h-6 w-full items-center gap-1.5 px-2.5 text-micro text-faint transition-colors duration-120 ease-settle -outline-offset-2 hover:bg-surface hover:text-muted"
          >
            <Icon
              name="chevron"
              className={`size-3 transition-transform duration-200 ease-settle ${endedOpen ? 'rotate-90' : ''}`}
            />
            Ended ({ended.length})
          </button>
        </li>
      )}
      {endedOpen && ended.map(renderCard)}
    </ul>
  );
}
