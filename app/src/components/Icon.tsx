import type { ReactNode } from 'react';
import type { SurfaceIcon } from '../lib/sessions.ts';

export type IconName = SurfaceIcon | 'bell-off' | 'pin' | 'chevron' | 'arrow-out' | 'gear';

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
  unknown: (
    <>
      <circle cx="8" cy="8" r="5.75" />
      <path d="M6.5 6.5a1.5 1.5 0 1 1 2.1 1.38c-.4.17-.6.5-.6.9v.22M8 10.9v.1" />
    </>
  ),
  'bell-off': (
    <path d="M4.5 10.5V7a3.5 3.5 0 0 1 5.6-2.8M11.5 7v3.5l1 1.25h-9M6.75 13.5a1.4 1.4 0 0 0 2.5 0M2.5 2.5l11 11" />
  ),
  pin: <path d="M5.5 2.25h5M6.75 2.25v4L4.5 9.5h7L9.25 6.25v-4M8 9.5v4.25" />,
  chevron: <path d="m6 4 4 4-4 4" />,
  gear: (
    <>
      <circle cx="8" cy="8" r="2" />
      <path d="M6.9 1.75h2.2l.35 1.6 1.2.7 1.55-.5 1.1 1.9-1.2 1.1v1.4l1.2 1.1-1.1 1.9-1.55-.5-1.2.7-.35 1.6H6.9l-.35-1.6-1.2-.7-1.55.5-1.1-1.9 1.2-1.1v-1.4l-1.2-1.1 1.1-1.9 1.55.5 1.2-.7Z" />
    </>
  ),
  'arrow-out': <path d="M9.25 2.75h4v4M13.25 2.75 7.5 8.5M11.25 9.75v2a1.5 1.5 0 0 1-1.5 1.5h-5.5a1.5 1.5 0 0 1-1.5-1.5v-5.5a1.5 1.5 0 0 1 1.5-1.5h2" />,
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
