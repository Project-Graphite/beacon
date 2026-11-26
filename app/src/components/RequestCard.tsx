import { useState } from 'react';
import type { PendingDecision } from '../beacon';
import { useDecision } from '../hooks/useDecision.ts';
import { useNow } from '../hooks/useNow.ts';
import { destructiveReason } from '../lib/destructive.ts';
import { requestId } from '../lib/queue.ts';
import { redactJson } from '../lib/redact.ts';
import { mcpTool, requestMeta } from '../lib/requests.ts';
import { DecisionMark } from './DecisionMark';
import { PlanRequest } from './PlanRequest';
import { QuestionRequest } from './QuestionRequest';
import { TextButton } from './TextButton';
import { ToolActions } from './ToolActions';
import { ToolRequestBody } from './ToolRequestBody';

interface RequestCardProps {
  request: PendingDecision;
  marked: boolean;
  currentId: number | null;
  keys: Map<number, (key: string) => void> | null;
  onJump: () => void;
}

export function RequestCard({ request, marked, currentId, keys, onJump }: RequestCardProps) {
  const now = useNow(true);
  const { outcome, failure, send } = useDecision(request);
  const [unmasked, setUnmasked] = useState(false);
  const destructive = request.kind === 'tool' ? destructiveReason(request.tool_name, request.tool_input, request.cwd) : null;
  const sending = outcome !== null;
  const title =
    request.kind === 'question' ? 'Question' : request.kind === 'plan' ? 'Plan' : mcpTool(request.tool_name) ? 'MCP tool' : request.tool_name;

  return (
    <div
      id={keys ? requestId(request.id) : undefined}
      className={`relative scroll-my-3 animate-appear rounded-card border bg-raised p-3 shadow-card ${destructive ? 'border-danger' : 'border-line'}`}
    >
      {marked && <span aria-hidden="true" className="absolute inset-y-2 -left-px w-0.5 rounded-full bg-wip" />}
      {outcome && (
        <span className="absolute top-1 right-1">
          <DecisionMark mark={outcome} />
        </span>
      )}
      <div className="flex items-baseline gap-2">
        <span className="min-w-0 truncate font-semibold text-ink">{title}</span>
        {JSON.stringify(redactJson('', [request.tool_input, request.plan])) !== JSON.stringify([request.tool_input, request.plan]) && (
          <span className="ml-auto">
            <TextButton onClick={() => setUnmasked(!unmasked)}>{unmasked ? 'Mask secrets' : 'Show unmasked'}</TextButton>
          </span>
        )}
      </div>
      <p className="mt-0.5 truncate text-micro text-faint tabular-nums">{requestMeta(request, now).join(' · ')}</p>
      {request.kind === 'tool' && (
        <>
          <ToolRequestBody request={request} masked={!unmasked} />
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
      {request.kind === 'question' && (
        <QuestionRequest request={request} masked={!unmasked} sending={sending} keys={keys} send={send} onJump={onJump} />
      )}
      {request.kind === 'plan' && (
        <PlanRequest request={request} masked={!unmasked} sending={sending} keys={keys} send={send} onJump={onJump} />
      )}
      {failure && <p role="alert" className="mt-1.5 text-micro text-danger [overflow-wrap:anywhere]">{failure}</p>}
    </div>
  );
}
