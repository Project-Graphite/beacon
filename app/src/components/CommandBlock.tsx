import { tokenizeCommand, type TokenKind } from '../lib/command.ts';
import { textLines } from '../lib/lines.ts';
import { LineClamp } from './LineClamp';

const TOKEN_CLASSES: Record<TokenKind, string | undefined> = {
  command: 'text-ink',
  word: 'text-ink/80',
  flag: 'text-muted',
  string: 'text-live/75',
  operator: 'text-wip',
  variable: 'text-highlight',
  comment: 'text-faint',
  space: undefined,
};

export function CommandBlock({ command }: { command: string }) {
  return (
    <LineClamp lines={textLines(command)}>
      {(shown) => (
        <pre className="mt-1 rounded-field bg-surface px-2 py-1.5 font-mono text-secondary whitespace-pre-wrap [overflow-wrap:anywhere]">
          {tokenizeCommand(shown.join('\n')).map((token, index) => (
            <span key={index} className={TOKEN_CLASSES[token.kind]}>
              {token.text}
            </span>
          ))}
        </pre>
      )}
    </LineClamp>
  );
}
