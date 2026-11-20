import { useOffscreenPause } from '../hooks/useOffscreenPause.ts';

export function Spinner() {
  const watched = useOffscreenPause<SVGSVGElement>();
  return (
    <svg
      ref={watched}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      aria-hidden="true"
      className="size-3 shrink-0 animate-turn text-ink"
    >
      <path d="M12.5 8A4.5 4.5 0 1 1 8 3.5" className="motion-reduce:hidden" />
      <circle cx="8" cy="8" r="4.5" className="hidden motion-reduce:inline" />
    </svg>
  );
}
