import type { DiffKind, DiffLine } from '../lib/diff.ts';
import { LineClamp } from './LineClamp';

const LINE_STYLES: Record<DiffKind, { mark: string; line: string; markColour: string }> = {
  context: { mark: ' ', line: 'text-muted', markColour: 'text-faint' },
  removed: { mark: '-', line: 'bg-danger/12 text-ink', markColour: 'text-danger' },
  added: { mark: '+', line: 'bg-live/12 text-ink', markColour: 'text-live' },
};

export function DiffBlock({ lines }: { lines: DiffLine[] }) {
  return (
    <LineClamp lines={lines}>
      {(shown) => (
        <div className="mt-1 overflow-hidden rounded-field bg-surface py-1 font-mono text-secondary">
          {shown.map((line, index) => (
            <div key={index} className={`flex pr-2 ${LINE_STYLES[line.kind].line}`}>
              <span aria-hidden="true" className={`w-5 shrink-0 text-center select-none ${LINE_STYLES[line.kind].markColour}`}>
                {LINE_STYLES[line.kind].mark}
              </span>
              <span className="min-w-0 whitespace-pre-wrap [overflow-wrap:anywhere]">{line.text || ' '}</span>
            </div>
          ))}
        </div>
      )}
    </LineClamp>
  );
}
