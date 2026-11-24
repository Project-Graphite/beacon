import { useState } from 'react';
import type { Step } from '../beacon';
import { formatStepDuration } from '../lib/format.ts';
import { recentSteps } from '../lib/steps.ts';
import { Collapse } from './Collapse';
import { Icon } from './Icon';
import { StepGlyph } from './StepGlyph';
import { StepLabel } from './StepLabel';
import { TextButton } from './TextButton';

const stepRow = (step: Step) => (
  <li key={step.id} className="flex h-6 items-center gap-2 text-secondary">
    <StepGlyph status={step.status} />
    <Icon name={step.kind} className="size-3 shrink-0 text-faint" />
    <span className="min-w-0 flex-1 truncate text-muted">
      <StepLabel label={step.label} />
    </span>
    <span className="shrink-0 text-micro text-faint tabular-nums">{formatStepDuration(step.duration_ms)}</span>
  </li>
);

export function ActivityList({ steps }: { steps: Step[] }) {
  const [showEarlier, setShowEarlier] = useState(false);
  const { earlier, recent } = recentSteps(steps);
  return (
    <section>
      <h3 className="mb-1.5 text-micro font-medium text-faint">Activity</h3>
      {earlier.length > 0 && !showEarlier && (
        <TextButton onClick={() => setShowEarlier(true)}>Show earlier ({earlier.length})</TextButton>
      )}
      <Collapse open={showEarlier}>
        <ol>{earlier.map(stepRow)}</ol>
      </Collapse>
      <ol>{recent.map(stepRow)}</ol>
    </section>
  );
}
