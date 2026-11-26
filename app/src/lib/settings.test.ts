import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { Overlay, OverlayDisplay } from '../beacon';
import {
  attentionSummary,
  avoidedLabel,
  behaviourSummary,
  CLEAR_CHOICES,
  commandMissing,
  displayLabel,
  hooksButtons,
  overlaySummary,
  percent,
  positionChoices,
  QUIT_CHOICES,
  REPEAT_CHOICES,
  spanLabel,
  splitPlan,
  stallMinutes,
  startupSummary,
  UI_DEFAULTS,
  uiSettings,
} from './settings.ts';

const OLD_SHELL = { hotkey: 'Ctrl+Shift+B', toasts_enabled: false, toast_on_finished: true, sounds_enabled: true };

test('settings the shell does not send yet read as their defaults', () => {
  assert.deepEqual(uiSettings(OLD_SHELL), {
    ...OLD_SHELL,
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
    pill_background: 'solid',
  });
  assert.deepEqual(uiSettings({}), UI_DEFAULTS);
});

test('settings the shell sends are kept, clamped to their ranges and limited to their choices', () => {
  const sent = {
    ...OLD_SHELL,
    visibility: 'never',
    reveal_on_hover: false,
    size: 'large',
    idle_opacity: 0.35,
    attention_style: 'colour',
    attention_repeat_seconds: 300,
    peek_seconds: 7,
    start_mode: 'manual',
    quit_when_idle_minutes: 0,
    pin_while_active: true,
    open_on_hover: false,
    pill_background: 'none',
  } as const;
  assert.deepEqual(uiSettings(sent), sent);
  const odd = uiSettings({
    idle_opacity: 0.05,
    peek_seconds: 30,
    attention_repeat_seconds: 45,
    quit_when_idle_minutes: 10,
    visibility: 'sometimes' as never,
  });
  assert.equal(odd.idle_opacity, 0.2);
  assert.equal(odd.peek_seconds, 10);
  assert.equal(odd.attention_repeat_seconds, 60);
  assert.equal(odd.quit_when_idle_minutes, 15);
  assert.equal(odd.visibility, 'active');
  assert.equal(uiSettings({ idle_opacity: 1.4 }).idle_opacity, 1);
  assert.equal(uiSettings({ idle_opacity: 0.333 }).idle_opacity, 0.35);
  assert.equal(uiSettings({ peek_seconds: 1 }).peek_seconds, 2);
  assert.equal(uiSettings({ peek_seconds: 4.4 }).peek_seconds, 4);
  assert.equal(uiSettings({ idle_opacity: Number.NaN }).idle_opacity, 0.6);
});

test('the segmented values read as short spans', () => {
  assert.deepEqual(
    REPEAT_CHOICES.map((choice) => [choice.value, choice.label]),
    [
      [0, 'Off'],
      [30, '30 s'],
      [60, '1 min'],
      [120, '2 min'],
      [300, '5 min'],
    ],
  );
  assert.deepEqual(
    QUIT_CHOICES.map((choice) => [choice.value, choice.label]),
    [
      [0, 'Never'],
      [5, '5 min'],
      [15, '15 min'],
      [60, '1 hour'],
    ],
  );
  assert.equal(spanLabel(7200, 'Never'), '2 hours');
  assert.equal(percent(0.6), '60%');
  assert.equal(percent(0.35), '35%');
});

test('only a missing command counts as missing', () => {
  assert.equal(commandMissing('Command preview_attention not found'), true);
  assert.equal(commandMissing('cannot play the preview'), false);
  assert.equal(commandMissing(new Error('Command preview_attention not found')), false);
});

const INSTALL_PLAN = `Settings: C:\\t\\claude\\settings.json
Helper:   C:\\b\\beacon-hook.exe
  copied to C:\\t\\home\\bin\\beacon-hook.exe
--- C:\\t\\claude\\settings.json
+++ C:\\t\\claude\\settings.json
@@ -1,3 +1,4 @@
 {
-  "model": "opus"
+  "model": "opus",
+  "hooks": {}
 }
Warning: the registry-delivered managed policy is not checked
Nothing was changed. Run again with --yes to install.
`;

test('a plan splits into its notes and its diff without the CLI hint or the warnings', () => {
  assert.deepEqual(splitPlan(INSTALL_PLAN), {
    notes: [
      'Settings: C:\\t\\claude\\settings.json',
      'Helper:   C:\\b\\beacon-hook.exe',
      '  copied to C:\\t\\home\\bin\\beacon-hook.exe',
    ],
    diff: [
      { kind: 'context', text: '@@ -1,3 +1,4 @@' },
      { kind: 'context', text: '{' },
      { kind: 'removed', text: '  "model": "opus"' },
      { kind: 'added', text: '  "model": "opus",' },
      { kind: 'added', text: '  "hooks": {}' },
      { kind: 'context', text: '}' },
    ],
  });
});

test('a plan without a diff is all notes', () => {
  assert.deepEqual(
    splitPlan('Settings: s.json\ns.json will be deleted: it did not exist before Beacon was installed.\n'),
    {
      notes: ['Settings: s.json', 's.json will be deleted: it did not exist before Beacon was installed.'],
      diff: [],
    },
  );
});

test('the buttons follow the hooks state', () => {
  assert.deepEqual(hooksButtons('not_installed'), [{ label: 'Install', action: 'install' }]);
  assert.deepEqual(hooksButtons('needs_repair'), [
    { label: 'Repair', action: 'install' },
    { label: 'Uninstall', action: 'uninstall' },
  ]);
  assert.deepEqual(hooksButtons('installed'), [{ label: 'Uninstall', action: 'uninstall' }]);
});

test('pinning while Claude is active is off by default and every other default is unchanged', () => {
  assert.deepEqual(UI_DEFAULTS, {
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
    pill_background: 'solid',
  });
  assert.equal(uiSettings({ pin_while_active: true }).pin_while_active, true);
});

const DISPLAYS: OverlayDisplay[] = [
  { id: 'a', name: 'XG27AQDMES', width: 2560, height: 1440, primary: true, target: false, avoided: true },
  { id: 'b', name: '24M2N3200', width: 1080, height: 1920, primary: false, target: false, avoided: false },
  { id: 'c', name: 'Display 3', width: 1920, height: 464, primary: false, target: true, avoided: false },
];

test('closed sections sum up their current values on one line', () => {
  const overlay: Overlay = { style: 'tab', position: 'right', displays: DISPLAYS };
  assert.equal(overlaySummary(UI_DEFAULTS, overlay), 'Edge tab · right edge · while Claude is active');
  assert.equal(
    overlaySummary({ ...UI_DEFAULTS, visibility: 'never', pin_while_active: true }, { ...overlay, style: 'pill', position: 'bottom-left' }),
    'Pill · bottom left · never · pinned while active',
  );
  assert.equal(overlaySummary(UI_DEFAULTS, null), 'while Claude is active');
  assert.equal(attentionSummary(UI_DEFAULTS), 'Pulse and label · every 1 min · notifications on');
  assert.equal(
    attentionSummary({ ...UI_DEFAULTS, attention_style: 'colour', attention_repeat_seconds: 0, toasts_enabled: false }),
    'Colour only · no reminders · notifications off',
  );
  assert.equal(startupSummary(UI_DEFAULTS), 'When Claude is used · closes after 15 min');
  assert.equal(startupSummary({ ...UI_DEFAULTS, quit_when_idle_minutes: 0 }), 'When Claude is used · stays open');
  assert.equal(startupSummary({ ...UI_DEFAULTS, quit_when_idle_minutes: 60 }), 'When Claude is used · closes after 1 hour');
  assert.equal(startupSummary({ ...UI_DEFAULTS, start_mode: 'login' }), 'When I sign in to Windows');
  assert.equal(
    behaviourSummary(UI_DEFAULTS, { stall_minutes: 10, answer_unattended: false, clear_finished_minutes: 10 }),
    'stalled 10 min · cleared 10 min · Ctrl+Alt+Space',
  );
  assert.equal(
    behaviourSummary({ ...UI_DEFAULTS, hotkey: 'Ctrl+Shift+B' }, { stall_minutes: 3, answer_unattended: true, clear_finished_minutes: 0 }),
    'headless answered · stalled 3 min · never cleared · Ctrl+Shift+B',
  );
  assert.equal(behaviourSummary(UI_DEFAULTS, null), 'Ctrl+Alt+Space');
});

test('displays read as their name and size, and the avoided ones as a name or a count', () => {
  assert.equal(displayLabel(DISPLAYS[2]), 'Display 3 (1920x464)');
  assert.equal(avoidedLabel(DISPLAYS), 'XG27AQDMES');
  assert.equal(avoidedLabel(DISPLAYS.map((display) => ({ ...display, avoided: false }))), 'None');
  assert.equal(avoidedLabel(DISPLAYS.map((display) => ({ ...display, avoided: true }))), '3 displays');
  assert.deepEqual(
    positionChoices('tab').map((choice) => choice.value),
    ['left', 'right', 'top', 'bottom'],
  );
  assert.deepEqual(
    positionChoices('pill').map((choice) => choice.value),
    ['top-left', 'top-right', 'bottom-left', 'bottom-right'],
  );
});

test('stall minutes are whole numbers from 1 to 240', () => {
  assert.equal(stallMinutes(' 15 '), 15);
  assert.equal(stallMinutes('1'), 1);
  assert.equal(stallMinutes('240'), 240);
  for (const text of ['', '0', '241', '1.5', '-3', 'ten', '1e2']) assert.equal(stallMinutes(text), null, text);
});

test('finished sessions clear after never, 5, 10 or 30 minutes or an hour', () => {
  assert.deepEqual(
    CLEAR_CHOICES.map((choice) => [choice.value, choice.label]),
    [
      [0, 'Never'],
      [5, '5 min'],
      [10, '10 min'],
      [30, '30 min'],
      [60, '1 hour'],
    ],
  );
  assert.equal(
    behaviourSummary(UI_DEFAULTS, { stall_minutes: 10, answer_unattended: false, clear_finished_minutes: 60 }),
    'stalled 10 min · cleared 1 hour · Ctrl+Alt+Space',
  );
});
