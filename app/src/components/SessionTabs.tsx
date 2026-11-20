import type { Session } from '../beacon';
import { needsInput, sessionTitle } from '../lib/sessions.ts';
import { Avatar } from './Avatar';

interface SessionTabsProps {
  tabs: Session[];
  currentId: string;
  onSwitch: (sessionId: string) => void;
}

export function SessionTabs({ tabs, currentId, onSwitch }: SessionTabsProps) {
  return (
    <div role="tablist" aria-label="Sessions" className="flex min-w-0 flex-1 items-center gap-0.5 overflow-hidden px-0.5">
      {tabs.map((tab) => (
        <button
          key={tab.session_id}
          type="button"
          role="tab"
          aria-selected={tab.session_id === currentId}
          title={`${sessionTitle(tab)} ([ and ] switch)`}
          onClick={() => onSwitch(tab.session_id)}
          className="press relative grid size-6 shrink-0 place-items-center rounded-[5px] enabled:hover:bg-line-soft"
        >
          <Avatar session={tab} size="tab" ringed={tab.session_id === currentId} />
          {needsInput(tab) && (
            <span aria-hidden="true" className="absolute top-0.5 right-0.5 size-[5px] rounded-full bg-wip" />
          )}
        </button>
      ))}
    </div>
  );
}
