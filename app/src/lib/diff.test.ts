import assert from 'node:assert/strict';
import { test } from 'node:test';
import { lineDiff, writePreview, type DiffLine } from './diff.ts';

const shown = (lines: DiffLine[]) =>
  lines.map((line) => `${line.kind === 'removed' ? '-' : line.kind === 'added' ? '+' : ' '}${line.text}`);

test('a changed line in the middle is removed then added between context', () => {
  assert.deepEqual(shown(lineDiff('a\nb\nc', 'a\nB\nc')), [' a', '-b', '+B', ' c']);
});

test('insertions and deletions keep the longest common lines', () => {
  assert.deepEqual(shown(lineDiff('one\ntwo\nthree\nfour', 'zero\none\nthree\nfour\nfive')), [
    '+zero',
    ' one',
    '-two',
    ' three',
    ' four',
    '+five',
  ]);
  assert.deepEqual(shown(lineDiff('x\na\ny\nb\nz', 'a\nb')), ['-x', ' a', '-y', ' b', '-z']);
});

test('empty sides are all added or all removed', () => {
  assert.deepEqual(shown(lineDiff('', 'new\nlines')), ['+new', '+lines']);
  assert.deepEqual(shown(lineDiff('old', '')), ['-old']);
  assert.deepEqual(lineDiff('', ''), []);
  assert.deepEqual(shown(lineDiff('same', 'same')), [' same']);
});

test('line endings and a final newline do not count as changes', () => {
  assert.deepEqual(shown(lineDiff('a\r\nb\r\n', 'a\nb')), [' a', ' b']);
});

test('every line of both sides appears once and in order', () => {
  const before = 'l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8';
  const after = 'l2\nX\nl4\nl3\nl5\nl8\nY';
  const lines = lineDiff(before, after);
  assert.deepEqual(
    lines.filter((line) => line.kind !== 'added').map((line) => line.text),
    before.split('\n'),
  );
  assert.deepEqual(
    lines.filter((line) => line.kind !== 'removed').map((line) => line.text),
    after.split('\n'),
  );
});

test('very large changes fall back to removed then added', () => {
  const before = Array.from({ length: 2500 }, (_, index) => `old ${index}`).join('\n');
  const after = Array.from({ length: 2500 }, (_, index) => `new ${index}`).join('\n');
  const lines = lineDiff(before, after);
  assert.equal(lines.length, 5000);
  assert.equal(lines[0].kind, 'removed');
  assert.equal(lines[2500].kind, 'added');
});

test('a written file previews its first twenty lines as added', () => {
  const content = Array.from({ length: 23 }, (_, index) => `line ${index + 1}`).join('\n') + '\n';
  const preview = writePreview(content);
  assert.equal(preview.lines.length, 20);
  assert.deepEqual(preview.lines[0], { kind: 'added', text: 'line 1' });
  assert.equal(preview.more, 3);
  assert.deepEqual(writePreview('short'), { lines: [{ kind: 'added', text: 'short' }], more: 0 });
  assert.deepEqual(writePreview(''), { lines: [], more: 0 });
});
