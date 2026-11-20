import { IconButton } from './IconButton';

interface SessionActionsProps {
  requestPending: boolean;
  onJump: () => void;
  onCrossOff: () => void;
}

export function SessionActions({ requestPending, onJump, onCrossOff }: SessionActionsProps) {
  return (
    <span className="flex shrink-0 items-center">
      <IconButton icon="arrow-out" title="Jump to window" onClick={onJump} />
      <IconButton
        icon="cross"
        title={requestPending ? 'Answer the request first' : 'Cross off until it is active again'}
        disabled={requestPending}
        onClick={onCrossOff}
      />
    </span>
  );
}
