import { useEffect, useState } from 'react';
import { decide, type Decision, type PendingDecision } from '../beacon';
import { useArmed } from '../hooks/useArmed.ts';
import { destructiveReason } from '../lib/destructive.ts';
import { requestId } from '../lib/queue.ts';
import { redact } from '../lib/redact.ts';
import { decisionMark } from '../lib/requests.ts';
import { DecisionMark } from './DecisionMark';
import { FilledButton } from './FilledButton';
import { OutlineButton } from './OutlineButton';

interface RequestRowProps {
  request: PendingDecision;
  marked: boolean;
  currentId: number | null;
  keys: Map<number, (key: string) => void> | null;
  onOpen: () => void;
}

export function RequestRow({ request, marked, currentId, keys, onOpen }: RequestRowProps) {
  const [outcome, setOutcome] = useState<'check' | 'cross' | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [armed, setArmed] = useArmed(currentId);
  const destructive = request.kind === 'tool' ? destructiveReason(request.tool_name, request.tool_input, request.cwd) : null;
  const line = failure ?? redact(request.preview);
  const send = (decision: Decision) => {
    setOutcome(decisionMark(decision));
    setFailure(null);
    decide(request.id, decision).catch((error) => {
      setOutcome(null);
      setFailure(String(error));
    });
  };
  const allow = () => {
    if (destructive && !armed) setArmed(true);
    else send({ type: 'allow' });
  };

  useEffect(() => {
    if (!keys) return;
    keys.set(request.id, (key) => {
      if (outcome) return;
      if (request.kind === 'tool' && key === 'a') allow();
      else if (request.kind === 'tool' && key === 'd') send({ type: 'deny' });
      else {
        onOpen();
        window.setTimeout(() => keys.get(request.id)?.(key));
      }
    });
    return () => void keys.delete(request.id);
  });

  return (
    <div
      id={keys ? requestId(request.id) : undefined}
      data-motion={`request:${request.id}`}
      className="relative flex h-7 scroll-my-2 items-center gap-2 pr-2.5 pl-10"
    >
      {marked && <span aria-hidden="true" className="absolute inset-y-1.5 left-[34px] w-0.5 rounded-full bg-wip" />}
      {destructive && (
        <span role="img" aria-label={`Looks destructive: ${destructive}`} title={`Looks destructive: ${destructive}`} className="size-1.5 shrink-0 rounded-full bg-danger" />
      )}
      <span
        title={line}
        className={`min-w-0 flex-1 truncate font-mono text-secondary ${failure ? 'text-danger' : 'text-ink'}`}
      >
        {line}
      </span>
      <span className="relative flex shrink-0 items-center justify-end">
        <span
          inert={outcome !== null}
          className={`flex items-center gap-1 transition-opacity duration-150 ease-settle ${outcome ? 'opacity-0' : ''}`}
        >
          {request.kind === 'tool' ? (
            <>
              <FilledButton compact onClick={allow}>
                {armed ? 'Press again' : 'Allow'}
              </FilledButton>
              <OutlineButton compact onClick={() => send({ type: 'deny' })}>
                Deny
              </OutlineButton>
            </>
          ) : (
            <OutlineButton compact onClick={onOpen}>
              {request.kind === 'question' ? 'Answer' : 'Review'}
            </OutlineButton>
          )}
        </span>
        {outcome && (
          <span className="absolute right-0">
            <DecisionMark mark={outcome} />
          </span>
        )}
      </span>
    </div>
  );
}
