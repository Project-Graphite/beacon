import { useEffect, useRef } from 'react';
import type { Session } from '../beacon';
import { needsInput, sessionTitle } from '../lib/sessions.ts';
import { Avatar } from './Avatar';

interface SessionTabsProps {
  tabs: Session[];
  currentId: string;
  onSwitch: (sessionId: string) => void;
}

export function SessionTabs({ tabs, currentId, onSwitch }: SessionTabsProps) {
  const strip = useRef<HTMLDivElement>(null);
  useEffect(() => {
    strip.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [currentId]);
  return (
    <div ref={strip} role="tablist" aria-label="Sessions" className="flex min-w-0 flex-1 items-center gap-0.5 scroll-px-1 overflow-x-auto p-1">
      {tabs.map((tab) => (
        <button
          key={tab.session_id}
          type="button"
          role="tab"
          aria-selected={tab.session_id === currentId}
          title={`${sessionTitle(tab)} ([ and ] switch)`}
          onClick={() => onSwitch(tab.session_id)}
          className="press relative grid size-7 shrink-0 place-items-center rounded-field enabled:hover:bg-surface"
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
