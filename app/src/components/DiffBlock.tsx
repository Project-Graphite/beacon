import type { DiffKind, DiffLine } from '../lib/diff.ts';

const LINE_STYLES: Record<DiffKind, { mark: string; line: string; markColour: string }> = {
  context: { mark: ' ', line: 'text-muted', markColour: 'text-faint' },
  removed: { mark: '-', line: 'bg-danger/12 text-ink', markColour: 'text-danger' },
  added: { mark: '+', line: 'bg-live/12 text-ink', markColour: 'text-live' },
};

export function DiffBlock({ lines }: { lines: DiffLine[] }) {
  return (
    <div className="scroll-thin mt-1 max-h-[252px] overflow-auto rounded-field bg-surface py-1 font-mono text-secondary">
      <div className="w-max min-w-full">
        {lines.map((line, index) => (
          <div key={index} className={`flex pr-2 ${LINE_STYLES[line.kind].line}`}>
            <span aria-hidden="true" className={`w-5 shrink-0 text-center select-none ${LINE_STYLES[line.kind].markColour}`}>
              {LINE_STYLES[line.kind].mark}
            </span>
            <span className="whitespace-pre">{line.text || ' '}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
