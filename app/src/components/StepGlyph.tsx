import { useState } from 'react';
import type { StepStatus } from '../beacon';
import { Icon, type IconName } from './Icon';
import { Spinner } from './Spinner';

const FINISHED: Record<Exclude<StepStatus, 'running'>, { icon: IconName; className: string; label: string }> = {
  done: { icon: 'check', className: 'text-muted', label: 'Done' },
  failed: { icon: 'cross', className: 'text-danger', label: 'Failed' },
  stopped: { icon: 'dash', className: 'text-faint', label: 'Stopped' },
};

export function StepGlyph({ status }: { status: StepStatus }) {
  const [initial] = useState(status);
  if (status === 'running') return <Spinner />;
  const { icon, className, label } = FINISHED[status];
  return (
    <span role="img" aria-label={label} className={`shrink-0 ${className} ${initial === 'running' ? 'animate-pop' : ''}`}>
      <Icon name={icon} className="block size-3" />
    </span>
  );
}
