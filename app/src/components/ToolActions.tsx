import { useEffect, useState } from 'react';
import type { Decision, PendingDecision } from '../beacon';
import { useArmed } from '../hooks/useArmed.ts';
import { suggestedRule } from '../lib/rules.ts';
import { FilledButton } from './FilledButton';
import { JumpButton } from './JumpButton';
import { KeyHint } from './KeyHint';
import { MessageField } from './MessageField';
import { OutlineButton } from './OutlineButton';
import { RuleEditor } from './RuleEditor';
import { TextButton } from './TextButton';

interface ToolActionsProps {
  request: PendingDecision;
  destructive: boolean;
  currentId: number | null;
  sending: boolean;
  keys: Map<number, (key: string) => void> | null;
  send: (decision: Decision) => void;
  onJump: () => void;
}

export function ToolActions({ request, destructive, currentId, sending, keys, send, onJump }: ToolActionsProps) {
  const [mode, setMode] = useState<'buttons' | 'rule' | 'message'>('buttons');
  const [armed, setArmed] = useArmed(currentId);
  const [more, setMore] = useState(false);

  const allow = () => {
    if (destructive && !armed) setArmed(true);
    else send({ type: 'allow' });
  };

  useEffect(() => {
    if (!keys) return;
    keys.set(request.id, (key) => {
      if (mode !== 'buttons' || sending) return;
      if (key === 'a') allow();
      if (key === 'd') send({ type: 'deny' });
      if (key === 'm') setMode('message');
    });
    return () => void keys.delete(request.id);
  });

  if (mode === 'rule') {
    const suggestion = suggestedRule(request.tool_name, request.permission_suggestions);
    return (
      <RuleEditor
        initialRule={suggestion.text}
        initialDestination={suggestion.destination}
        disabled={sending}
        onSave={(update) => send({ type: 'allow_always', permissions: [update] })}
        onCancel={() => setMode('buttons')}
      />
    );
  }
  if (mode === 'message') {
    return (
      <MessageField
        label="Message for Claude"
        placeholder="Why not, or what to do instead"
        sendLabel="Deny"
        disabled={sending}
        onSend={(message) => send({ type: 'deny', message })}
        onCancel={() => setMode('buttons')}
      />
    );
  }
  return (
    <div className="mt-2 flex flex-wrap items-center gap-1.5">
      <FilledButton disabled={sending} onClick={allow}>
        {armed ? 'Press again to allow' : 'Allow'}
        <KeyHint shortcut="A" />
      </FilledButton>
      {!destructive && (
        <OutlineButton disabled={sending} onClick={() => setMode('rule')}>
          Always allow…
        </OutlineButton>
      )}
      <OutlineButton disabled={sending} onClick={() => send({ type: 'deny' })}>
        Deny
        <KeyHint shortcut="D" />
      </OutlineButton>
      <OutlineButton disabled={sending} onClick={() => setMode('message')}>
        Deny with message
        <KeyHint shortcut="M" />
      </OutlineButton>
      {more ? (
        <OutlineButton disabled={sending} onClick={() => send({ type: 'deny_and_stop' })}>
          <span className="text-danger">Deny and stop Claude</span>
        </OutlineButton>
      ) : (
        <TextButton disabled={sending} onClick={() => setMore(true)}>
          More
        </TextButton>
      )}
      <JumpButton shortcut onJump={onJump} />
    </div>
  );
}
