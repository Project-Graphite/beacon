import { detail, ended, hooks, overlay, snapshot, ui } from './fixture';
import { emit } from './tauri-event';

const settings = { ...ui };

function hide(sessionId: string, dismissed: boolean) {
  snapshot.sessions = snapshot.sessions.map((session) => (session.session_id === sessionId ? { ...session, dismissed } : session));
  emit('beacon://snapshot', snapshot);
}

const answers: Record<string, (args: Record<string, unknown>) => unknown> = {
  get_detail: ({ sessionId }) => detail(String(sessionId)),
  get_ended: () => ended,
  hooks_status: () => hooks,
  hooks_plan: () => 'No change to the settings file.',
  hooks_apply: () => hooks,
  app_info: () => ({ version: '0.2.0', home: 'C:\\Users\\you\\AppData\\Local\\Beacon' }),
  get_overlay: () => overlay,
  set_ui_settings: ({ change }) => Object.assign(settings, change),
  dismiss: ({ sessionId }) => hide(String(sessionId), true),
  undismiss: ({ sessionId }) => hide(String(sessionId), false),
  decide: ({ id }) => {
    snapshot.pending = snapshot.pending.filter((request) => request.id !== id);
    emit('beacon://snapshot', snapshot);
  },
  jump: () => ({ result: 'focused', kind: { type: 'editor', scheme: 'vscode' }, ambiguous: false, link_opened: true }),
  send_prompt_to_window: () => ({
    path: 'wake',
    outcome: { result: 'focused', kind: null, ambiguous: false, link_opened: false },
  }),
};

export async function invoke(command: string, args: Record<string, unknown> = {}) {
  return answers[command]?.(args);
}
