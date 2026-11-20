import { useState, type ReactNode } from 'react';
import type { Session } from '../beacon';
import { useOffscreenPause } from '../hooks/useOffscreenPause.ts';
import { avatarFace, avatarHue, blinkDelay, type AvatarFace } from '../lib/avatar.ts';
import { StateMark } from './StateMark';

const SIZES = { tab: 14, row: 20, header: 36 } as const;

const STROKE = { fill: 'none', stroke: 'var(--color-paper)', strokeWidth: 1.5, strokeLinecap: 'round' } as const;

const FLAT_MOUTH = <path d="M8.5 13.75h3" {...STROKE} />;

const eyes = (draw: (x: number) => ReactNode) => (
  <>
    {draw(7)}
    {draw(13)}
  </>
);

function face(kind: AvatarFace, sessionId: string) {
  switch (kind) {
    case 'working':
      return (
        <>
          <g className="origin-center animate-blink [transform-box:fill-box]" style={{ animationDelay: `${blinkDelay(sessionId)}ms` }}>
            {eyes((x) => <circle cx={x} cy="9" r="1.6" />)}
          </g>
          {FLAT_MOUTH}
        </>
      );
    case 'alarmed':
      return (
        <>
          {eyes((x) => <ellipse cx={x} cy="8.5" rx="1.7" ry="2.4" />)}
          <circle cx="10" cy="14.25" r="1.25" />
        </>
      );
    case 'happy':
      return <path d="M5.4 9.75q1.6-2.3 3.2 0M11.4 9.75q1.6-2.3 3.2 0M7.5 12.5q2.5 2.9 5 0" {...STROKE} />;
    case 'crossed':
      return (
        <>
          <path d="M5.8 7.8l2.4 2.4M8.2 7.8l-2.4 2.4M11.8 7.8l2.4 2.4M14.2 7.8l-2.4 2.4" {...STROKE} />
          {FLAT_MOUTH}
        </>
      );
    case 'sideways':
      return (
        <>
          {eyes((x) => <circle cx={x + 1.4} cy="9" r="1.6" />)}
          <path d="M9.75 13.75h3" {...STROKE} />
        </>
      );
    case 'sleepy':
      return eyes((x) => <path d={`M${x - 1.8} 9h3.6a1.8 1.8 0 0 1-3.6 0Z`} />);
    case 'blank':
      return eyes((x) => <circle cx={x} cy="9" r="1.6" />);
    case 'closed':
      return <path d="M5.4 9.5h3.2M11.4 9.5h3.2" {...STROKE} />;
  }
}

interface AvatarProps {
  session: Session;
  size: keyof typeof SIZES;
  ringed?: boolean;
}

export function Avatar({ session, size, ringed }: AvatarProps) {
  const kind = avatarFace(session.state);
  const [faces, setFaces] = useState({ kind, entries: 0 });
  if (faces.kind !== kind) setFaces({ kind, entries: faces.entries + 1 });
  const watched = useOffscreenPause<HTMLSpanElement>();
  const pixels = SIZES[size];
  return (
    <span
      ref={watched}
      style={{ borderRadius: pixels / 4 }}
      className={`relative block shrink-0 ${ringed ? 'outline-1 outline-offset-1 outline-ink' : ''} ${kind === 'closed' ? 'opacity-45 grayscale' : ''}`}
    >
      <svg
        key={faces.entries}
        viewBox="0 0 20 20"
        width={pixels}
        height={pixels}
        aria-hidden="true"
        className={`block ${kind === 'alarmed' && faces.entries > 0 ? 'animate-wiggle' : ''}`}
      >
        <rect width="20" height="20" rx="5" fill={avatarHue(session.session_id)} />
        <g fill="var(--color-paper)">{face(kind, session.session_id)}</g>
      </svg>
      {size !== 'tab' && (
        <span className="absolute -right-[5px] -bottom-[5px] rounded-full bg-paper p-[1.5px]">
          <StateMark state={session.state} snoozed={session.attention && !session.alerting} />
        </span>
      )}
    </span>
  );
}
