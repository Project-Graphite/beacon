import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseInline, parseMarkdown, plainText } from './markdown.ts';

const text = (value: string) => ({ type: 'text', text: value });

test('headings of every level keep their inline content and drop closing hashes', () => {
  assert.deepEqual(parseMarkdown('# One\n### Three **bold** ##\n###### Six'), [
    { type: 'heading', level: 1, children: [text('One')] },
    { type: 'heading', level: 3, children: [text('Three '), { type: 'strong', children: [text('bold')] }] },
    { type: 'heading', level: 6, children: [text('Six')] },
  ]);
  assert.deepEqual(parseMarkdown('#hashtag'), [{ type: 'paragraph', children: [text('#hashtag')] }]);
});

test('paragraphs are split by blank lines and keep their line breaks', () => {
  assert.deepEqual(parseMarkdown('first line\nsecond line\n\n\nnext paragraph'), [
    { type: 'paragraph', children: [text('first line\nsecond line')] },
    { type: 'paragraph', children: [text('next paragraph')] },
  ]);
});

test('unordered and ordered lists keep one nesting level', () => {
  assert.deepEqual(parseMarkdown('- one\n- two\n  - two a\n    - deeper\n* three'), [
    {
      type: 'list',
      ordered: false,
      start: 1,
      items: [
        { children: [text('one')], nested: null },
        {
          children: [text('two')],
          nested: {
            type: 'list',
            ordered: false,
            start: 1,
            items: [
              { children: [text('two a')], nested: null },
              { children: [text('deeper')], nested: null },
            ],
          },
        },
        { children: [text('three')], nested: null },
      ],
    },
  ]);
  assert.deepEqual(parseMarkdown('3. c\n4) d\n   1. d1\n\n5. e\n\nafter'), [
    {
      type: 'list',
      ordered: true,
      start: 3,
      items: [
        { children: [text('c')], nested: null },
        {
          children: [text('d')],
          nested: { type: 'list', ordered: true, start: 1, items: [{ children: [text('d1')], nested: null }] },
        },
        { children: [text('e')], nested: null },
      ],
    },
    { type: 'paragraph', children: [text('after')] },
  ]);
});

test('a list of another kind starts a new list and continuation lines join the item', () => {
  assert.deepEqual(parseMarkdown('- a\n  wrapped\n1. b'), [
    { type: 'list', ordered: false, start: 1, items: [{ children: [text('a\nwrapped')], nested: null }] },
    { type: 'list', ordered: true, start: 1, items: [{ children: [text('b')], nested: null }] },
  ]);
});

test('fenced code blocks keep their text verbatim', () => {
  assert.deepEqual(parseMarkdown('before\n```ts\nconst a = **1**;\n  indented\n```\nafter'), [
    { type: 'paragraph', children: [text('before')] },
    { type: 'code', text: 'const a = **1**;\n  indented' },
    { type: 'paragraph', children: [text('after')] },
  ]);
  assert.deepEqual(parseMarkdown('~~~~\n```\n~~~~'), [{ type: 'code', text: '```' }]);
});

test('an unterminated fence runs to the end of the text', () => {
  assert.deepEqual(parseMarkdown('intro\n```\nline 1\n\n# not a heading'), [
    { type: 'paragraph', children: [text('intro')] },
    { type: 'code', text: 'line 1\n\n# not a heading' },
  ]);
});

test('inline code, bold, italic and links nest inside text', () => {
  assert.deepEqual(parseInline('run `npm *test*` then **bold _and italic_** or *it* see [docs](https://x.test/a)'), [
    text('run '),
    { type: 'code', text: 'npm *test*' },
    text(' then '),
    { type: 'strong', children: [text('bold '), { type: 'emphasis', children: [text('and italic')] }] },
    text(' or '),
    { type: 'emphasis', children: [text('it')] },
    text(' see '),
    { type: 'link', children: [text('docs')], url: 'https://x.test/a' },
  ]);
  assert.deepEqual(parseInline('__strong__ and *a **b** c*'), [
    { type: 'strong', children: [text('strong')] },
    text(' and '),
    { type: 'emphasis', children: [text('a '), { type: 'strong', children: [text('b')] }, text(' c')] }],
  );
});

test('unmatched or intraword markers stay plain text', () => {
  for (const value of ['snake_case_name', '2 * 3 * 4', '**open', 'a ` b', '[text] (url)', '[x](has space)']) {
    assert.deepEqual(parseInline(value), [text(value)]);
  }
  assert.deepEqual(parseInline('\\*not italic\\*'), [text('*not italic*')]);
});

test('HTML-looking text stays text', () => {
  assert.deepEqual(parseMarkdown('<script>alert(1)</script>\n<img src=x onerror=alert(1)>'), [
    { type: 'paragraph', children: [text('<script>alert(1)</script>\n<img src=x onerror=alert(1)>')] },
  ]);
  assert.deepEqual(parseInline('[<b>x</b>](javascript:alert(1))'), [
    { type: 'link', children: [text('<b>x</b>')], url: 'javascript:alert(1' },
    text(')'),
  ]);
});

test('plain text drops the markup and keeps the words', () => {
  assert.equal(
    plainText('## Done\n\nFixed **the** `cache`, see [docs](https://x.test).\n\n- a\n  - b\n\n```\nnpm test'),
    'Done Fixed the cache, see docs. a b npm test',
  );
});

test('unknown syntax falls through as plain text', () => {
  assert.deepEqual(parseMarkdown('> quoted\n| a | b |\n---'), [
    { type: 'paragraph', children: [text('> quoted\n| a | b |\n---')] },
  ]);
  assert.deepEqual(parseMarkdown(''), []);
});
