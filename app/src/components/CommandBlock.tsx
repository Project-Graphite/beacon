import { useOffscreenPause } from '../hooks/useOffscreenPause.ts';
import { tokenizeCommand, type TokenKind } from '../lib/command.ts';
import { textLines } from '../lib/lines.ts';
import { LineClamp } from './LineClamp';

const TOKEN_CLASSES: Record<TokenKind, string | undefined> = {
  command: 'text-ink',
  word: 'text-ink/80',
  flag: 'text-muted',
  string: 'text-live',
  operator: 'text-wip',
  variable: 'text-highlight',
  comment: 'text-faint',
  space: undefined,
};

export function CommandBlock({ command, live }: { command: string; live?: boolean }) {
  const watched = useOffscreenPause<HTMLPreElement>();
  return (
    <LineClamp lines={textLines(command)}>
      {(shown) => (
        <pre
          ref={watched}
          className="mt-1.5 rounded-field border border-line-soft bg-surface px-2.5 py-2 font-mono text-secondary whitespace-pre-wrap [overflow-wrap:anywhere]"
        >
          {live && <span className="text-faint">$ </span>}
          {tokenizeCommand(shown.join('\n')).map((token, index) => (
            <span key={index} className={TOKEN_CLASSES[token.kind]}>
              {token.text}
            </span>
          ))}
          {live && (
            <span aria-hidden="true" className="ml-0.5 inline-block h-[1.1em] w-[0.5em] animate-caret bg-ink/70 align-[-0.2em]" />
          )}
        </pre>
      )}
    </LineClamp>
  );
}
