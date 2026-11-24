import assert from 'node:assert/strict';
import { test } from 'node:test';
import { recordShortcut, shortcutKeys, type KeyPress } from './shortcut.ts';

const press = (key: string, code: string, held: Partial<KeyPress> = {}): KeyPress => ({
  key,
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...held,
});

test('a key held with modifiers is recorded in accelerator form', () => {
  assert.deepEqual(recordShortcut(press('b', 'KeyB', { ctrlKey: true, altKey: true })), { kind: 'shortcut', text: 'Ctrl+Alt+B' });
  assert.deepEqual(recordShortcut(press(' ', 'Space', { ctrlKey: true, shiftKey: true })), { kind: 'shortcut', text: 'Ctrl+Shift+Space' });
  assert.deepEqual(recordShortcut(press('ArrowUp', 'ArrowUp', { metaKey: true, shiftKey: true })), { kind: 'shortcut', text: 'Shift+Super+Up' });
  assert.deepEqual(recordShortcut(press('@', 'Digit2', { ctrlKey: true, shiftKey: true })), { kind: 'shortcut', text: 'Ctrl+Shift+2' });
});

test('function keys need one modifier and other keys need two', () => {
  assert.deepEqual(recordShortcut(press('F9', 'F9', { ctrlKey: true })), { kind: 'shortcut', text: 'Ctrl+F9' });
  assert.equal(recordShortcut(press('F9', 'F9')).kind, 'invalid');
  assert.equal(recordShortcut(press('c', 'KeyC', { ctrlKey: true })).kind, 'invalid');
  assert.equal(recordShortcut(press(' ', 'Space', { altKey: true })).kind, 'invalid');
});

test('modifiers alone keep recording and Escape cancels', () => {
  assert.deepEqual(recordShortcut(press('Control', 'ControlLeft', { ctrlKey: true })), { kind: 'pending' });
  assert.deepEqual(recordShortcut(press('Escape', 'Escape')), { kind: 'cancel' });
});

test('a plain or Shift-only key is refused', () => {
  assert.equal(recordShortcut(press('b', 'KeyB')).kind, 'invalid');
  assert.equal(recordShortcut(press('B', 'KeyB', { shiftKey: true })).kind, 'invalid');
  assert.equal(recordShortcut(press('Unidentified', 'IntlRo', { ctrlKey: true })).kind, 'invalid');
});

test('a shortcut splits into readable keys', () => {
  assert.deepEqual(shortcutKeys('Ctrl+Alt+B'), ['Ctrl', 'Alt', 'B']);
  assert.deepEqual(shortcutKeys('Shift+Super+BracketLeft'), ['Shift', 'Win', '[']);
});
