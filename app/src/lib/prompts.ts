import type { Prompt, PromptPath, Session, SessionState } from '../beacon';

const TURN_STATES: SessionState[] = [
  'starting',
  'working',
  'needs_permission',
  'needs_answer',
  'needs_plan_approval',
  'needs_mcp_input',
  'stalled',
];

const PATH_LABELS: Record<PromptPath, string> = {
  at_turn_end: 'Sends when the current turn finishes.',
  wake: 'Sends now. Claude sees it as a follow-up from Beacon, not as a typed message.',
  prefill: 'Opens the session in VS Code with the text in its prompt box and on your clipboard. If VS Code says the session is already open, paste it.',
  copy_and_jump: "Copies the text and jumps to the session's window. Paste it there.",
};

export const promptBoxId = (sessionId: string) => `prompt-${sessionId}`;

export function promptPath(session: Session): { path: PromptPath; label: string } | null {
  if (session.state === 'ended') return null;
  const path: PromptPath = TURN_STATES.includes(session.state)
    ? 'at_turn_end'
    : session.can_wake
      ? 'wake'
      : session.entrypoint === 'claude-vscode'
        ? 'prefill'
        : 'copy_and_jump';
  return { path, label: PATH_LABELS[path] };
}

export function recentPrompts(prompts: Prompt[]): Prompt[] {
  return [...prompts].sort((a, b) => b.id - a.id);
}

export function promptStatus(prompt: Prompt): string {
  return prompt.reason ? `${prompt.status}: ${prompt.reason}` : prompt.status;
}

export function queuedPrompts(session: Session): number {
  return session.prompts.filter((prompt) => prompt.status === 'queued').length;
}
