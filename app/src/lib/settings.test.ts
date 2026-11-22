import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  commandMissing,
  hooksButtons,
  percent,
  QUIT_CHOICES,
  REPEAT_CHOICES,
  spanLabel,
  splitPlan,
  stallMinutes,
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

test('stall minutes are whole numbers from 1 to 240', () => {
  assert.equal(stallMinutes(' 15 '), 15);
  assert.equal(stallMinutes('1'), 1);
  assert.equal(stallMinutes('240'), 240);
  for (const text of ['', '0', '241', '1.5', '-3', 'ten', '1e2']) assert.equal(stallMinutes(text), null, text);
});
