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
    <>
      {groups.map((group) => (
        <section key={group.label} className="border-line-soft not-first:border-t">
          {groups.length > 1 && <h2 className="px-3 pt-2 text-micro text-faint">{group.label}</h2>}
          <ul aria-label={group.label} className="divide-y divide-line-soft">
            {group.sessions.map(renderCard)}
          </ul>
        </section>
      ))}
      {ended.length > 0 && (
        <section className="border-line-soft not-first:border-t">
          <button
            type="button"
            aria-expanded={endedOpen}
            onClick={onToggleEnded}
            className="flex w-full items-center gap-1.5 px-3 py-2 text-micro text-faint transition-colors duration-150 ease-settle -outline-offset-2 hover:bg-surface hover:text-muted"
          >
            <Icon
              name="chevron"
              className={`size-3 transition-transform duration-150 ease-settle ${endedOpen ? 'rotate-90' : ''}`}
            />
            Ended ({ended.length})
          </button>
          {endedOpen && (
            <ul aria-label="Ended" className="divide-y divide-line-soft border-t border-line-soft">
              {ended.map(renderCard)}
            </ul>
          )}
        </section>
      )}
    </>
  );
}
