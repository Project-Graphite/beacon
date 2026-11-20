import { Icon } from './Icon';

export function DecisionMark({ mark }: { mark: 'check' | 'cross' }) {
  return (
    <span
      role="img"
      aria-label={mark === 'check' ? 'Sent' : 'Denied'}
      className={`grid size-[22px] shrink-0 animate-pop place-items-center ${mark === 'check' ? 'text-live' : 'text-muted'}`}
    >
      <Icon name={mark} className="size-3.5" />
    </span>
  );
}
