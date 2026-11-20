import assert from 'node:assert/strict';
import { test } from 'node:test';
import { hooksButtons, splitPlan, stallMinutes } from './settings.ts';

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
