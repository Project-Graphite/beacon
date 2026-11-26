import { createContext, useContext, useState } from 'react';
import { decide, type Decision, type PendingDecision } from '../beacon';
import { errorText } from '../lib/format.ts';
import { decisionMark } from '../lib/requests.ts';

export const RequestGate = createContext<(id: number) => boolean>(() => true);

export function useDecision(request: PendingDecision) {
  const [outcome, setOutcome] = useState<'check' | 'cross' | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const settled = useContext(RequestGate);

  const send = (decision: Decision) => {
    if (!settled(request.id)) return;
    setOutcome(decisionMark(decision));
    setFailure(null);
    decide(request.id, decision).catch((error) => {
      setOutcome(null);
      setFailure(errorText(error));
    });
  };

  return { outcome, failure, send };
}
