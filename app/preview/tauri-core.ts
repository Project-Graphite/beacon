import { version } from '../package.json';
import type { Session } from '../src/beacon';
import { promptPath } from '../src/lib/prompts.ts';
import { detail, ended, hooks, overlay, snapshot, ui } from './fixture';
import { emit, listen } from './tauri-event';

const settings = { ...ui };

Object.assign(window, { __beaconPreview: { emit, listen, snapshot } });

function update(sessionId: string, change: (session: (typeof snapshot.sessions)[number]) => Partial<(typeof snapshot.sessions)[number]>) {
  snapshot.sessions = snapshot.sessions.map((session) => (session.session_id === sessionId ? { ...session, ...change(session) } : session));
  emit('beacon://snapshot', snapshot);
}

const answers: Record<string, (args: Record<string, unknown>) => unknown> = {
  get_detail: ({ sessionId }) => detail(String(sessionId)),
  get_ended: () => ended,
  hooks_status: () => hooks,
  hooks_plan: () => 'No change to the settings file.',
  hooks_apply: () => hooks,
  app_info: () => ({ version, home: 'C:\\Users\\you\\AppData\\Local\\Beacon' }),
  get_overlay: () => overlay,
  set_ui_settings: ({ change }) => Object.assign(settings, change),
  set_panel_height: ({ height }) => (window.frameElement as HTMLElement | null)?.style.setProperty('height', `${height}px`),
  dismiss: ({ sessionId }) => update(String(sessionId), () => ({ dismissed: true })),
  undismiss: ({ sessionId }) => update(String(sessionId), () => ({ dismissed: false })),
  decide: ({ id }) => {
    const request = snapshot.pending.find((pending) => pending.id === id);
    window.setTimeout(() => {
      snapshot.pending = snapshot.pending.filter((pending) => pending !== request);
      update(String(request?.session_id), () => ({ state: 'working', alerting: false, attention: false }));
    }, 400);
  },
  queue_prompt: ({ sessionId, text }) =>
    update(String(sessionId), (session) => ({
      prompts: [
        ...session.prompts,
        {
          id: Date.now(),
          preview: String(text),
          created_ms: Date.now(),
          status: promptPath(session as Session)?.path === 'wake' ? 'delivered' : 'queued',
          reason: null,
        },
      ],
    })),
  jump: () => ({ result: 'focused', kind: { type: 'editor', scheme: 'vscode' }, ambiguous: false, link_opened: true }),
  send_prompt_to_window: () => ({
    path: 'wake',
    outcome: { result: 'focused', kind: null, ambiguous: false, link_opened: false },
  }),
};

export async function invoke(command: string, args: Record<string, unknown> = {}) {
  return answers[command]?.(args);
}
