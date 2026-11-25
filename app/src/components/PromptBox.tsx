import { useState } from 'react';
import { cancelPrompt, queuePrompt, sendPromptToWindow, type Session } from '../beacon';
import { errorText } from '../lib/format.ts';
import { jumpReport, prefillReport, type JumpReport } from '../lib/jump.ts';
import { promptBoxId, promptPath, promptStatus, recentPrompts } from '../lib/prompts.ts';
import { FilledButton } from './FilledButton';
import { TextButton } from './TextButton';

interface PromptBoxProps {
  session: Session;
  onJumpReport: (report: JumpReport) => void;
}

export function PromptBox({ session, onJumpReport }: PromptBoxProps) {
  const [text, setText] = useState('');
  const [sending, setSending] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [sentText, setSentText] = useState<{ id: number; text: string } | null>(null);
  const [mountedAt] = useState(Date.now);
  const path = promptPath(session);
  const prompts = recentPrompts(session.prompts);
  if (!path && prompts.length === 0) return null;

  const send = () => {
    if (!path || !text.trim() || sending) return;
    setSending(true);
    setFailure(null);
    const sent =
      path.path === 'at_turn_end' || path.path === 'wake'
        ? queuePrompt(session.session_id, text).then(() => true)
        : sendPromptToWindow(session.session_id, text).then((result) => {
            const report = result.path === 'prefill' ? prefillReport(result.outcome) : jumpReport(result.outcome);
            onJumpReport(report);
            return report.succeeded;
          });
    sent
      .then(
        (succeeded) => {
          if (!succeeded) return;
          setSentText({ id: Date.now(), text });
          setText('');
        },
        (error) => setFailure(errorText(error)),
      )
      .finally(() => setSending(false));
  };

  return (
    <section>
      {path && (
        <>
          <div className="relative">
            <textarea
              id={promptBoxId(session.session_id)}
              aria-label="Prompt for this session"
              placeholder="Send a prompt"
              value={text}
              readOnly={sending}
              onChange={(event) => setText(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
                  event.preventDefault();
                  send();
                }
              }}
              className="block max-h-[calc(6lh+18px)] min-h-[calc(2lh+18px)] w-full resize-none rounded-[10px] border border-line bg-raised px-3 py-2 text-ink shadow-card transition-colors duration-150 ease-settle field-sizing-content placeholder:text-faint focus:border-faint focus:outline-none"
            />
            {sentText && (
              <p
                key={sentText.id}
                aria-hidden="true"
                className="pointer-events-none absolute inset-x-0 top-0 animate-sent truncate border border-transparent px-3 py-2 text-ink motion-reduce:hidden"
              >
                {sentText.text}
              </p>
            )}
          </div>
          <div className="mt-2 flex items-center gap-3">
            <p className="min-w-0 flex-1 text-micro text-muted">{path.label}</p>
            <FilledButton disabled={sending || !text.trim()} onClick={send}>
              Send
            </FilledButton>
          </div>
        </>
      )}
      {failure && <p role="alert" className="mt-1 text-micro text-danger [overflow-wrap:anywhere]">{failure}</p>}
      {prompts.length > 0 && (
        <ol className="mt-3 space-y-1.5">
          {prompts.map((prompt) => (
            <li
              key={prompt.id}
              className={`min-w-0 ${prompt.created_ms > mountedAt ? 'origin-top-left animate-appear' : ''}`}
            >
              <p className="truncate text-secondary text-ink">{prompt.preview}</p>
              <p className="flex items-baseline gap-2 text-micro">
                <span className={`min-w-0 ${prompt.status === 'failed' ? 'text-danger' : 'text-muted'}`}>
                  {promptStatus(prompt)}
                </span>
                {prompt.status === 'queued' && (
                  <TextButton onClick={() => cancelPrompt(prompt.id).catch((error) => setFailure(errorText(error)))}>
                    Cancel
                  </TextButton>
                )}
              </p>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
