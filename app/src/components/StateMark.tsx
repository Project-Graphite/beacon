import { useState } from 'react';
import type { SessionState } from '../beacon';
import { useOffscreenPause } from '../hooks/useOffscreenPause.ts';

const RING = { fill: 'none', strokeWidth: 1.25, cx: 8, cy: 8 } as const;

function shape(state: SessionState) {
  switch (state) {
    case 'starting':
      return <circle cx="8" cy="8" r="2" className="fill-dormant" />;
    case 'working':
      return <path d="M12 8A4 4 0 1 1 8 4" fill="none" strokeWidth="1.5" strokeLinecap="round" className="stroke-ink" />;
    case 'needs_permission':
    case 'needs_answer':
    case 'needs_plan_approval':
    case 'needs_mcp_input':
      return <circle cx="8" cy="8" r="4.5" className="fill-wip" />;
    case 'stalled':
      return (
        <>
          <rect x="4.5" y="4" width="2.5" height="8" className="fill-wip" />
          <rect x="9" y="4" width="2.5" height="8" className="fill-wip" />
        </>
      );
    case 'done':
      return (
        <path
          d="M4.5 8.2 7 10.6l4.5-5"
          fill="none"
          strokeWidth="1.75"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="stroke-live"
        />
      );
    case 'error':
      return <path d="M3.5 12h9L8 4Z" className="fill-danger" />;
    case 'idle':
      return <circle {...RING} r="3.5" className="stroke-muted" />;
    case 'ended':
      return <circle {...RING} r="3.5" className="stroke-dormant" />;
  }
}

const glyph = (state: SessionState) => (
  <svg
    viewBox="0 0 16 16"
    aria-hidden="true"
    className={`absolute inset-0 size-full ${state === 'working' ? 'animate-turn' : ''}`}
  >
    {shape(state)}
  </svg>
);

export function StateMark({ state, snoozed }: { state: SessionState; snoozed: boolean }) {
  const [marks, setMarks] = useState<{ state: SessionState; previous: SessionState | null }>({ state, previous: null });
  if (marks.state !== state) setMarks({ state, previous: marks.state });
  const watched = useOffscreenPause<HTMLSpanElement>();
  return (
    <span ref={watched} className={`relative block size-2.5 shrink-0 ${snoozed ? 'opacity-45' : ''}`}>
      {marks.previous && (
        <span
          key={`${marks.previous}-${marks.state}`}
          className="absolute inset-0 animate-fade-out motion-reduce:hidden"
          onAnimationEnd={(event) => event.target === event.currentTarget && setMarks({ state, previous: null })}
        >
          {glyph(marks.previous)}
        </span>
      )}
      <span key={state} className={`absolute inset-0 ${marks.previous ? 'animate-pop' : ''}`}>
        {glyph(state)}
      </span>
      {state.startsWith('needs_') && !snoozed && (
        <span className="absolute inset-[2px] animate-halo rounded-full border border-wip motion-reduce:hidden" />
      )}
    </span>
  );
}
