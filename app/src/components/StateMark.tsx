import type { SessionState } from '../beacon';

const RING = { fill: 'none', strokeWidth: 1.25, cx: 8, cy: 8 } as const;

function shape(state: SessionState, snoozed: boolean) {
  switch (state) {
    case 'starting':
      return <circle cx="8" cy="8" r="2" className="fill-dormant" />;
    case 'working':
      return (
        <>
          <path
            d="M12 8A4 4 0 1 1 8 4"
            fill="none"
            strokeWidth="1.5"
            strokeLinecap="round"
            className="origin-[8px_8px] animate-turn stroke-ink motion-reduce:hidden"
          />
          <circle {...RING} r="4" strokeWidth="1.5" className="hidden stroke-ink motion-reduce:inline" />
        </>
      );
    case 'needs_permission':
    case 'needs_answer':
    case 'needs_plan_approval':
    case 'needs_mcp_input':
      return (
        <>
          {!snoozed && <circle {...RING} r="4.5" className="animate-halo stroke-wip motion-reduce:hidden" />}
          <circle cx="8" cy="8" r="4.5" className="fill-wip" />
        </>
      );
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

export function StateMark({ state, snoozed }: { state: SessionState; snoozed: boolean }) {
  return (
    <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" className={snoozed ? 'opacity-45' : undefined}>
      {shape(state, snoozed)}
    </svg>
  );
}
