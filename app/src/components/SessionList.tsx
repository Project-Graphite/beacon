import type { ReactNode } from 'react';
import type { Session } from '../beacon';
import type { SessionGroup } from '../lib/sessions.ts';
import { FoldedGroup } from './FoldedGroup';

interface SessionListProps {
  groups: SessionGroup[];
  ended: Session[];
  crossed: Session[];
  endedOpen: boolean;
  crossedOpen: boolean;
  onToggleEnded: () => void;
  onToggleCrossed: () => void;
  renderRow: (session: Session) => ReactNode;
  renderCrossedRow: (session: Session) => ReactNode;
}

export function SessionList({
  groups,
  ended,
  crossed,
  endedOpen,
  crossedOpen,
  onToggleEnded,
  onToggleCrossed,
  renderRow,
  renderCrossedRow,
}: SessionListProps) {
  return (
    <ul aria-label="Sessions" className="session-list">
      {groups.flatMap((group) => [
        groups.length > 1 && (
          <li key={`group-${group.label}`} data-group data-motion={`group:${group.label}`} className="flex h-5 items-center px-2.5">
            <h2 className="text-micro text-faint">{group.label}</h2>
          </li>
        ),
        ...group.sessions.map(renderRow),
      ])}
      {ended.length > 0 && (
        <FoldedGroup key="ended" motionId="ended" label={`Ended (${ended.length})`} open={endedOpen} onToggle={onToggleEnded}>
          {ended.map(renderRow)}
        </FoldedGroup>
      )}
      {crossed.length > 0 && (
        <FoldedGroup
          key="crossed"
          motionId="crossed"
          label={`Crossed off (${crossed.length})`}
          open={crossedOpen}
          onToggle={onToggleCrossed}
        >
          {crossed.map(renderCrossedRow)}
        </FoldedGroup>
      )}
    </ul>
  );
}
