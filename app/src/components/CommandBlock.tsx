import { useOffscreenPause } from '../hooks/useOffscreenPause.ts';
import { commandWords, tokenizeCommand, type TokenKind } from '../lib/command.ts';
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
          className="mt-1.5 rounded-field border border-line-soft bg-surface px-2.5 py-2 font-mono text-secondary break-all whitespace-pre-wrap"
        >
          {live && <span className="text-faint">$ </span>}
          {commandWords(tokenizeCommand(shown.join('\n'))).map((word, index) =>
            word[0].kind === 'space' ? (
              word[0].text
            ) : (
              <span key={index} className="inline-block max-w-full">
                {word.map((token, part) => (
                  <span key={part} className={TOKEN_CLASSES[token.kind]}>
                    {token.text}
                  </span>
                ))}
              </span>
            ),
          )}
          {live && (
            <span aria-hidden="true" className="ml-0.5 inline-block h-[1.1em] w-[0.5em] animate-caret bg-ink/70 align-[-0.2em]" />
          )}
        </pre>
      )}
    </LineClamp>
  );
}
