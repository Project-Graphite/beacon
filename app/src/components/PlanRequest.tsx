import { useEffect, useState } from 'react';
import type { Decision, PendingDecision } from '../beacon';
import { textLines } from '../lib/lines.ts';
import { redact } from '../lib/redact.ts';
import { FilledButton } from './FilledButton';
import { JumpButton } from './JumpButton';
import { KeyHint } from './KeyHint';
import { LineClamp } from './LineClamp';
import { Markdown } from './Markdown';
import { MessageField } from './MessageField';
import { OutlineButton } from './OutlineButton';

const PLAN_WAIT_MS = 5000;
const KEEP_PLANNING = 'The user wants to keep planning.';

interface PlanRequestProps {
  request: PendingDecision;
  masked: boolean;
  sending: boolean;
  keys: Map<number, (key: string) => void> | null;
  send: (decision: Decision) => void;
  onJump: () => void;
}

export function PlanRequest({ request, masked, sending, keys, send, onJump }: PlanRequestProps) {
  const [gaveUp, setGaveUp] = useState(() => Date.now() - request.created_ms >= PLAN_WAIT_MS);
  const [feedback, setFeedback] = useState(false);

  useEffect(() => {
    if (gaveUp || request.plan !== null) return;
    const timer = window.setTimeout(() => setGaveUp(true), PLAN_WAIT_MS - (Date.now() - request.created_ms));
    return () => window.clearTimeout(timer);
  }, [gaveUp, request.plan, request.created_ms]);

  const approve = () => send({ type: 'allow_with_input', input: request.tool_input });
  const keepPlanning = () => send({ type: 'deny', message: KEEP_PLANNING });

  useEffect(() => {
    if (!keys) return;
    keys.set(request.id, (key) => {
      if (sending || feedback) return;
      if (key === 'a' && request.plan !== null) approve();
      if (key === 'd') keepPlanning();
      if (key === 'm') setFeedback(true);
    });
    return () => void keys.delete(request.id);
  });

  return (
    <>
      {request.plan !== null ? (
        <LineClamp lines={textLines(masked ? redact(request.plan) : request.plan)}>
          {(shown) => (
            <div className="mt-1 rounded-field bg-surface px-2 py-1.5">
              <Markdown source={shown.join('\n')} />
            </div>
          )}
        </LineClamp>
      ) : (
        <p className="mt-1 text-secondary text-muted">
          {gaveUp ? 'The plan text is not available here; read it in the session window.' : 'Loading the plan…'}
        </p>
      )}
      {feedback ? (
        <MessageField
          label="Feedback on the plan"
          placeholder="What should change"
          sendLabel="Reject"
          disabled={sending}
          onSend={(message) => send({ type: 'deny', message })}
          onCancel={() => setFeedback(false)}
        />
      ) : (
        <div className="mt-2 flex flex-wrap items-center gap-1.5">
          <FilledButton disabled={sending || request.plan === null} onClick={approve}>
            Approve plan
            <KeyHint shortcut="A" />
          </FilledButton>
          <OutlineButton disabled={sending} onClick={keepPlanning}>
            Keep planning
            <KeyHint shortcut="D" />
          </OutlineButton>
          <OutlineButton disabled={sending} onClick={() => setFeedback(true)}>
            Reject with feedback
            <KeyHint shortcut="M" />
          </OutlineButton>
          <JumpButton shortcut onJump={onJump} />
        </div>
      )}
    </>
  );
}
