import { useState } from 'react';
import { decide, type Decision, type PendingDecision } from '../beacon';
import { destructiveReason } from '../lib/destructive.ts';
import { requestId } from '../lib/queue.ts';
import { redact } from '../lib/redact.ts';
import { mcpTool, requestMeta } from '../lib/requests.ts';
import { PlanRequest } from './PlanRequest';
import { QuestionRequest } from './QuestionRequest';
import { TextButton } from './TextButton';
import { ToolActions } from './ToolActions';
import { ToolRequestBody } from './ToolRequestBody';

interface RequestCardProps {
  request: PendingDecision;
  now: number;
  marked: boolean;
  currentId: number | null;
  keys: Map<number, (key: string) => void>;
  onJump: () => void;
}

const unchanged = (text: string) => text;

export function RequestCard({ request, now, marked, currentId, keys, onJump }: RequestCardProps) {
  const [sending, setSending] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [unmasked, setUnmasked] = useState(false);
  const destructive = request.kind === 'tool' ? destructiveReason(request.tool_name, request.tool_input, request.cwd) : null;
  const rawInput = JSON.stringify(request.tool_input) ?? '';
  const send = (decision: Decision) => {
    setSending(true);
    setFailure(null);
    decide(request.id, decision).catch((error) => {
      setSending(false);
      setFailure(String(error));
    });
  };
  const title =
    request.kind === 'question' ? 'Question' : request.kind === 'plan' ? 'Plan' : mcpTool(request.tool_name) ? 'MCP tool' : request.tool_name;

  return (
    <div
      id={requestId(request.id)}
      className={`relative mt-2 scroll-my-2 rounded-field border bg-paper p-2 ${destructive ? 'border-danger' : 'border-line'}`}
    >
      {marked && <span aria-hidden="true" className="absolute inset-y-2 -left-px w-0.5 rounded-full bg-wip" />}
      <div className="flex items-baseline gap-2">
        <span className="min-w-0 truncate font-semibold text-ink">{title}</span>
        {request.kind === 'tool' && redact(rawInput) !== rawInput && (
          <span className="ml-auto">
            <TextButton onClick={() => setUnmasked(!unmasked)}>{unmasked ? 'Mask secrets' : 'Show unmasked'}</TextButton>
          </span>
        )}
      </div>
      <p className="truncate font-mono text-micro text-faint">{requestMeta(request, now).join(' · ')}</p>
      {request.kind === 'tool' && (
        <>
          <ToolRequestBody request={request} mask={unmasked ? unchanged : redact} />
          {destructive && <p className="mt-1.5 text-micro text-danger">Looks destructive: {destructive}</p>}
          <ToolActions
            request={request}
            destructive={destructive !== null}
            currentId={currentId}
            sending={sending}
            keys={keys}
            send={send}
            onJump={onJump}
          />
        </>
      )}
      {request.kind === 'question' && <QuestionRequest request={request} sending={sending} keys={keys} send={send} onJump={onJump} />}
      {request.kind === 'plan' && <PlanRequest request={request} sending={sending} keys={keys} send={send} onJump={onJump} />}
      {failure && <p className="mt-1.5 text-micro text-danger [overflow-wrap:anywhere]">{failure}</p>}
    </div>
  );
}
