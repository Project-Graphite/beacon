import type { ReactNode } from 'react';
import type { StepKind } from '../beacon';
import type { SurfaceIcon } from '../lib/sessions.ts';

export type IconName =
  | SurfaceIcon
  | StepKind
  | 'bell-off'
  | 'chevron'
  | 'arrow-out'
  | 'gear'
  | 'check'
  | 'cross'
  | 'box'
  | 'dash'
  | 'sound-on'
  | 'sound-off';

const QUESTION = (
  <>
    <circle cx="8" cy="8" r="5.75" />
    <path d="M6.5 6.5a1.5 1.5 0 1 1 2.1 1.38c-.4.17-.6.5-.6.9v.22M8 10.9v.1" />
  </>
);

const SPEAKER = 'M2.5 6.25h2.25L8 3.5v9L4.75 9.75H2.5Z';

const PATHS: Record<IconName, ReactNode> = {
  terminal: (
    <>
      <rect x="2" y="2.75" width="12" height="10.5" rx="1.5" />
      <path d="M5 6.25 7 8l-2 1.75M8.5 10.25H11" />
    </>
  ),
  headless: <path d="M8 1.75 13.5 4.75v6.5L8 14.25l-5.5-3v-6.5ZM2.5 4.75 8 7.75l5.5-3M8 7.75v6.5" />,
  editor: <path d="M5.5 4.5 2 8l3.5 3.5M10.5 4.5 14 8l-3.5 3.5M9 3 7 13" />,
  desktop: (
    <>
      <rect x="1.75" y="2.5" width="12.5" height="8.5" rx="1.5" />
      <path d="M8 11v2.75M5.5 13.75h5" />
    </>
  ),
  unknown: QUESTION,
  question: QUESTION,
  'bell-off': (
    <path d="M4.5 10.5V7a3.5 3.5 0 0 1 5.6-2.8M11.5 7v3.5l1 1.25h-9M6.75 13.5a1.4 1.4 0 0 0 2.5 0M2.5 2.5l11 11" />
  ),
  chevron: <path d="m6 4 4 4-4 4" />,
  check: <path d="m3.5 8.5 3 3 6-7" />,
  cross: <path d="m4.5 4.5 7 7M11.5 4.5l-7 7" />,
  box: <rect x="3.75" y="3.75" width="8.5" height="8.5" rx="2" />,
  dash: <path d="M4.5 8h7" />,
  gear: (
    <>
      <circle cx="8" cy="8" r="2" />
      <path d="M6.9 1.75h2.2l.35 1.6 1.2.7 1.55-.5 1.1 1.9-1.2 1.1v1.4l1.2 1.1-1.1 1.9-1.55-.5-1.2.7-.35 1.6H6.9l-.35-1.6-1.2-.7-1.55.5-1.1-1.9 1.2-1.1v-1.4l-1.2-1.1 1.1-1.9 1.55.5 1.2-.7Z" />
    </>
  ),
  'arrow-out': <path d="M9.25 2.75h4v4M13.25 2.75 7.5 8.5M11.25 9.75v2a1.5 1.5 0 0 1-1.5 1.5h-5.5a1.5 1.5 0 0 1-1.5-1.5v-5.5a1.5 1.5 0 0 1 1.5-1.5h2" />,
  'sound-on': <path d={`${SPEAKER}M10.75 6a2.75 2.75 0 0 1 0 4M12.5 4.25a5.25 5.25 0 0 1 0 7.5`} />,
  'sound-off': <path d={`${SPEAKER}M10.5 6l3 4M13.5 6l-3 4`} />,
  prompt: <path d="M2.75 3.25h10.5v7.5H7.5l-3 2.5v-2.5H2.75Z" />,
  read: (
    <>
      <path d="M1.75 8S4 3.75 8 3.75 14.25 8 14.25 8 12 12.25 8 12.25 1.75 8 1.75 8Z" />
      <circle cx="8" cy="8" r="1.75" />
    </>
  ),
  edit: <path d="M10.5 2.75 13.25 5.5 6 12.75H3.25V10ZM9 4.25l2.75 2.75" />,
  run: <path d="M3 4.5 6.5 8 3 11.5M8.5 11.5h4.5" />,
  search: (
    <>
      <circle cx="7" cy="7" r="4.25" />
      <path d="m10.25 10.25 3.5 3.5" />
    </>
  ),
  web: (
    <>
      <circle cx="8" cy="8" r="5.75" />
      <path d="M2.25 8h11.5M8 2.25c-2.25 2.5-2.25 9 0 11.5M8 2.25c2.25 2.5 2.25 9 0 11.5" />
    </>
  ),
  agent: (
    <>
      <circle cx="8" cy="5" r="2.5" />
      <path d="M3.5 13.5c0-2.5 2-4.25 4.5-4.25s4.5 1.75 4.5 4.25" />
    </>
  ),
  task: (
    <>
      <rect x="2.75" y="2.75" width="10.5" height="10.5" rx="2" />
      <path d="m5.5 8 1.75 1.75L10.5 6.5" />
    </>
  ),
  mcp: <path d="M6 2v3M10 2v3M4.5 5h7v2.5a3.5 3.5 0 0 1-7 0ZM8 11v3" />,
  permission: <path d="M8 1.75 13 3.75v4c0 3-2.2 5.2-5 6.5-2.8-1.3-5-3.5-5-6.5v-4Z" />,
  plan: <path d="M6 4.5h7M6 8h7M6 11.5h7M3 4.5h.01M3 8h.01M3 11.5h.01" />,
  notice: (
    <>
      <circle cx="8" cy="8" r="5.75" />
      <path d="M8 7.5v3.5M8 5v.1" />
    </>
  ),
  finish: <path d="M3.5 14V2.5M3.5 3h8.5l-2 3 2 3H3.5" />,
  error: <path d="M8 2.5 14 13H2ZM8 6.75v2.75M8 11.25v.1" />,
};

export function Icon({ name, className }: { name: IconName; className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      {PATHS[name]}
    </svg>
  );
}
