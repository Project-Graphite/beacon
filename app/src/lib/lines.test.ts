import assert from 'node:assert/strict';
import { test } from 'node:test';
import { clampLines, textLines } from './lines.ts';

const numbered = (count: number) => Array.from({ length: count }, (_, index) => `line ${index + 1}`);

test('text splits into lines on LF or CRLF and a trailing newline adds no empty line', () => {
  assert.deepEqual(textLines('one'), ['one']);
  assert.deepEqual(textLines('one\ntwo\r\nthree'), ['one', 'two', 'three']);
  assert.deepEqual(textLines('one\ntwo\n'), ['one', 'two']);
  assert.deepEqual(textLines('one\r\n'), ['one']);
  assert.deepEqual(textLines('one\n\n'), ['one', '']);
  assert.deepEqual(textLines(''), ['']);
});

test('a block of up to 40 lines shows whole and a taller one shows its first 40 until expanded', () => {
  assert.equal(clampLines(numbered(40), false).length, 40);
  assert.deepEqual(clampLines(numbered(41), false), numbered(40));
  assert.deepEqual(clampLines(numbered(300), false), numbered(40));
  assert.deepEqual(clampLines(numbered(300), true), numbered(300));
  assert.deepEqual(clampLines(textLines(numbered(45).join('\n')), false).at(-1), 'line 40');
});
