import assert from 'node:assert/strict';
import { test } from 'node:test';
import { displayPath, relativeInside, shortenPath } from './paths.ts';

test('long paths are shortened from the left at a separator', () => {
  assert.equal(shortenPath('C:\\work', 20), 'C:\\work');
  assert.equal(shortenPath('C:\\STUFF\\Project-Graphite\\beacon\\spikes\\out\\m3', 24), '…\\beacon\\spikes\\out\\m3');
  assert.equal(shortenPath('/home/someone/projects/beacon', 16), '…/beacon');
  assert.equal(shortenPath('C:\\a-very-long-single-folder-name', 12), '…folder-name');
  assert.ok(shortenPath('C:\\STUFF\\Project-Graphite\\beacon\\spikes', 24).length <= 24);
});

test('paths inside the working directory are shown relative to it', () => {
  assert.equal(relativeInside('C:\\work\\src\\a.ts', 'C:\\work'), 'src\\a.ts');
  assert.equal(relativeInside('c:/Work/src/a.ts', 'C:\\work\\'), 'src/a.ts');
  assert.equal(relativeInside('notes.txt', 'C:\\work'), 'notes.txt');
  assert.equal(relativeInside('C:\\work\\.\\x\\..\\y.txt', 'C:\\work'), 'y.txt');
  assert.equal(relativeInside('C:\\work\\..\\other\\y.txt', 'C:\\work'), null);
  assert.equal(relativeInside('C:\\workshop\\y.txt', 'C:\\work'), null);
  assert.equal(relativeInside('D:\\work\\y.txt', 'C:\\work'), null);
  assert.equal(displayPath('C:\\work\\src\\a.ts', 'C:\\work'), 'src\\a.ts');
  assert.equal(displayPath('D:\\elsewhere\\a.ts', 'C:\\work'), 'D:\\elsewhere\\a.ts');
  assert.equal(displayPath('C:\\work', 'C:\\work'), 'C:\\work');
  assert.equal(displayPath('C:\\work\\a.ts', null), 'C:\\work\\a.ts');
});
