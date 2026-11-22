import type {
  HooksAction,
  HooksState,
  LiveConfig,
  Overlay,
  OverlayDisplay,
  OverlayPosition,
  OverlayStyle,
  UiSettings,
} from '../beacon';
import type { DiffLine } from './diff.ts';

export interface Choice<T> {
  value: T;
  label: string;
  help?: string;
}

export const UI_DEFAULTS: UiSettings = {
  hotkey: 'Ctrl+Alt+Space',
  toasts_enabled: true,
  toast_on_finished: true,
  sounds_enabled: false,
  visibility: 'active',
  reveal_on_hover: true,
  size: 'medium',
  idle_opacity: 0.6,
  attention_style: 'pulse_peek',
  attention_repeat_seconds: 60,
  peek_seconds: 4,
  start_mode: 'claude',
  quit_when_idle_minutes: 15,
  pin_while_active: false,
  open_on_hover: true,
};

export const OPACITY_RANGE = { min: 0.2, max: 1, step: 0.05 };
export const PEEK_RANGE = { min: 2, max: 10, step: 1 };
const REPEAT_SECONDS = [0, 30, 60, 120, 300];
const QUIT_MINUTES = [0, 5, 15, 60];
const CLEAR_MINUTES = [0, 5, 10, 30, 60];

export function spanLabel(seconds: number, none: string): string {
  if (seconds === 0) return none;
  if (seconds < 60) return `${seconds} s`;
  if (seconds < 3600) return `${seconds / 60} min`;
  const hours = seconds / 3600;
  return `${hours} ${hours === 1 ? 'hour' : 'hours'}`;
}

export const percent = (fraction: number) => `${Math.round(fraction * 100)}%`;

export const VISIBILITY_CHOICES: Choice<UiSettings['visibility']>[] = [
  { value: 'always', label: 'Always', help: 'Stays on its screen edge all the time.' },
  { value: 'active', label: 'While Claude is active', help: 'While Claude is working or needs you; hides once every session is idle.' },
  { value: 'attention', label: 'When Claude needs you', help: 'Appears when a session is waiting on you.' },
  { value: 'never', label: 'Never', help: 'Tray icon only: open the panel from the tray icon or the shortcut.' },
];

export const STYLE_CHOICES: Choice<OverlayStyle>[] = [
  { value: 'tab', label: 'Edge tab' },
  { value: 'pill', label: 'Pill' },
];

export const EDGE_CHOICES: Choice<OverlayPosition>[] = [
  { value: 'left', label: 'Left edge' },
  { value: 'right', label: 'Right edge' },
  { value: 'top', label: 'Top edge' },
  { value: 'bottom', label: 'Bottom edge' },
];

export const CORNER_CHOICES: Choice<OverlayPosition>[] = [
  { value: 'top-left', label: 'Top left' },
  { value: 'top-right', label: 'Top right' },
  { value: 'bottom-left', label: 'Bottom left' },
  { value: 'bottom-right', label: 'Bottom right' },
];

export const positionChoices = (style: OverlayStyle) => (style === 'tab' ? EDGE_CHOICES : CORNER_CHOICES);

export const displayLabel = ({ name, width, height }: OverlayDisplay) => `${name} (${width}x${height})`;

export function avoidedLabel(displays: OverlayDisplay[]): string {
  const avoided = displays.filter((display) => display.avoided);
  if (avoided.length === 0) return 'None';
  return avoided.length === 1 ? avoided[0].name : `${avoided.length} displays`;
}

export const SIZE_CHOICES: Choice<UiSettings['size']>[] = [
  { value: 'small', label: 'Small' },
  { value: 'medium', label: 'Medium' },
  { value: 'large', label: 'Large' },
];

export const ATTENTION_CHOICES: Choice<UiSettings['attention_style']>[] = [
  { value: 'colour', label: 'Colour only' },
  { value: 'pulse', label: 'Pulse' },
  { value: 'pulse_peek', label: 'Pulse and label' },
];

export const START_CHOICES: Choice<UiSettings['start_mode']>[] = [
  { value: 'claude', label: 'When Claude is used', help: 'Opens by itself when a Claude session starts.' },
  { value: 'login', label: 'When I sign in to Windows' },
  { value: 'manual', label: 'Only when I open it' },
];

export const REPEAT_CHOICES: Choice<number>[] = REPEAT_SECONDS.map((value) => ({ value, label: spanLabel(value, 'Off') }));

export const QUIT_CHOICES: Choice<number>[] = QUIT_MINUTES.map((value) => ({
  value,
  label: spanLabel(value * 60, 'Never'),
}));

export const CLEAR_CHOICES: Choice<number>[] = CLEAR_MINUTES.map((value) => ({
  value,
  label: spanLabel(value * 60, 'Never'),
}));

const oneOf = <T>(choices: Choice<T>[], value: unknown, fallback: T): T =>
  choices.find((choice) => choice.value === value)?.value ?? fallback;

const within = ({ min, max, step }: { min: number; max: number; step: number }, value: unknown, fallback: number) =>
  typeof value === 'number' && Number.isFinite(value)
    ? Math.min(max, Math.max(min, Math.round(value / step) * step))
    : fallback;

export function uiSettings(raw: Partial<UiSettings>): UiSettings {
  return {
    ...UI_DEFAULTS,
    ...raw,
    visibility: oneOf(VISIBILITY_CHOICES, raw.visibility, UI_DEFAULTS.visibility),
    reveal_on_hover: raw.reveal_on_hover ?? UI_DEFAULTS.reveal_on_hover,
    size: oneOf(SIZE_CHOICES, raw.size, UI_DEFAULTS.size),
    idle_opacity: Number(within(OPACITY_RANGE, raw.idle_opacity, UI_DEFAULTS.idle_opacity).toFixed(2)),
    attention_style: oneOf(ATTENTION_CHOICES, raw.attention_style, UI_DEFAULTS.attention_style),
    attention_repeat_seconds: oneOf(REPEAT_CHOICES, raw.attention_repeat_seconds, UI_DEFAULTS.attention_repeat_seconds),
    peek_seconds: within(PEEK_RANGE, raw.peek_seconds, UI_DEFAULTS.peek_seconds),
    start_mode: oneOf(START_CHOICES, raw.start_mode, UI_DEFAULTS.start_mode),
    quit_when_idle_minutes: oneOf(QUIT_CHOICES, raw.quit_when_idle_minutes, UI_DEFAULTS.quit_when_idle_minutes),
  };
}

export const labelOf = <T>(choices: Choice<T>[], value: T) =>
  choices.find((choice) => choice.value === value)?.label ?? '';

const lowered = (text: string) => text.charAt(0).toLowerCase() + text.slice(1);

export function overlaySummary(ui: UiSettings, overlay: Overlay | null): string {
  return [
    overlay && labelOf(STYLE_CHOICES, overlay.style),
    overlay?.position && lowered(labelOf(positionChoices(overlay.style), overlay.position)),
    lowered(labelOf(VISIBILITY_CHOICES, ui.visibility)),
    ui.pin_while_active && 'pinned while active',
  ]
    .filter(Boolean)
    .join(' · ');
}

export function attentionSummary(ui: UiSettings): string {
  return [
    labelOf(ATTENTION_CHOICES, ui.attention_style),
    ui.attention_repeat_seconds === 0 ? 'no reminders' : `every ${spanLabel(ui.attention_repeat_seconds, '')}`,
    `notifications ${ui.toasts_enabled ? 'on' : 'off'}`,
  ].join(' · ');
}

export function startupSummary(ui: UiSettings): string {
  const start = labelOf(START_CHOICES, ui.start_mode);
  if (ui.start_mode !== 'claude') return start;
  return `${start} · ${ui.quit_when_idle_minutes === 0 ? 'stays open' : `closes after ${lowered(labelOf(QUIT_CHOICES, ui.quit_when_idle_minutes))}`}`;
}

export function behaviourSummary(ui: UiSettings, config: LiveConfig | null): string {
  return [
    config && `Headless runs ${config.answer_unattended ? 'answered here' : 'pass through'}`,
    config && `stalled after ${config.stall_minutes} min`,
    config &&
      (config.clear_finished_minutes === 0
        ? 'finished kept'
        : `finished cleared after ${spanLabel(config.clear_finished_minutes * 60, '')}`),
    ui.hotkey,
  ]
    .filter(Boolean)
    .join(' · ');
}

export const commandMissing = (error: unknown) => /^command \S+ not found$/i.test(String(error));

export const HOOKS_STATES: Record<HooksState, { label: string; dot: string }> = {
  installed: { label: 'Installed', dot: 'bg-live' },
  not_installed: { label: 'Not installed', dot: 'bg-muted' },
  needs_repair: { label: 'Needs repair', dot: 'bg-wip' },
};

export function hooksButtons(state: HooksState): { label: string; action: HooksAction }[] {
  return [
    ...(state === 'installed'
      ? []
      : [{ label: state === 'needs_repair' ? 'Repair' : 'Install', action: 'install' as const }]),
    ...(state === 'not_installed' ? [] : [{ label: 'Uninstall', action: 'uninstall' as const }]),
  ];
}

const DIFF_MARKS: Record<string, DiffLine['kind']> = { ' ': 'context', '+': 'added', '-': 'removed' };

export function splitPlan(plan: string): { notes: string[]; diff: DiffLine[] } {
  const notes: string[] = [];
  const diff: DiffLine[] = [];
  let inDiff = false;
  for (const line of plan.replace(/\n$/, '').split('\n')) {
    if (line.startsWith('--- ') && !inDiff) {
      inDiff = true;
    } else if (inDiff && line.startsWith('+++ ')) {
      continue;
    } else if (inDiff && line.startsWith('@@')) {
      diff.push({ kind: 'context', text: line });
    } else if (inDiff && line[0] in DIFF_MARKS) {
      diff.push({ kind: DIFF_MARKS[line[0]], text: line.slice(1) });
    } else if (!line.startsWith('Nothing was changed.') && !line.startsWith('Warning: ')) {
      inDiff = false;
      notes.push(line);
    }
  }
  return { notes, diff };
}

export function stallMinutes(text: string): number | null {
  const trimmed = text.trim();
  if (!/^\d+$/.test(trimmed)) return null;
  const minutes = Number(trimmed);
  return minutes >= 1 && minutes <= 240 ? minutes : null;
}
