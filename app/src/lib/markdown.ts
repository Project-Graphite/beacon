export type Inline =
  | { type: 'text'; text: string }
  | { type: 'code'; text: string }
  | { type: 'strong'; children: Inline[] }
  | { type: 'emphasis'; children: Inline[] }
  | { type: 'link'; children: Inline[]; url: string };

export interface ListItem {
  children: Inline[];
  nested: ListBlock | null;
}

export interface ListBlock {
  type: 'list';
  ordered: boolean;
  start: number;
  items: ListItem[];
}

export type Block =
  | { type: 'heading'; level: number; children: Inline[] }
  | { type: 'paragraph'; children: Inline[] }
  | { type: 'code'; text: string }
  | ListBlock;

interface DraftList {
  ordered: boolean;
  start: number;
  items: { lines: string[]; nested: DraftList | null }[];
}

const FENCE = /^ {0,3}(`{3,}|~{3,})/;
const CLOSING_FENCE = /^ {0,3}(`{3,}|~{3,})[ \t]*$/;
const HEADING = /^ {0,3}(#{1,6})(?:[ \t]+(.*))?$/;
const LIST_ITEM = /^( *)([-*+]|\d{1,9}[.)])[ \t]+(.*)$/;
const ESCAPABLE = /[!-/:-@[-`{-~]/;

const INLINE_RULES: { pattern: RegExp; build: (match: RegExpExecArray) => Inline; wordBound?: boolean }[] = [
  { pattern: /^(`+)([^`][\s\S]*?)\1(?!`)/, build: (match) => ({ type: 'code', text: match[2] }) },
  {
    pattern: /^\[([^\]\n]+)\]\(([^)\s]+)\)/,
    build: (match) => ({ type: 'link', children: parseInline(match[1]), url: match[2] }),
  },
  {
    pattern: /^\*\*(?=[^\s*])([\s\S]*?[^\s*])\*\*/,
    build: (match) => ({ type: 'strong', children: parseInline(match[1]) }),
  },
  {
    pattern: /^__(?=[^\s_])([\s\S]*?[^\s_])__(?!\w)/,
    build: (match) => ({ type: 'strong', children: parseInline(match[1]) }),
    wordBound: true,
  },
  {
    pattern: /^\*(?=[^\s*])([\s\S]*?[^\s*])\*(?!\*)/,
    build: (match) => ({ type: 'emphasis', children: parseInline(match[1]) }),
  },
  {
    pattern: /^_(?=[^\s_])([\s\S]*?[^\s_])_(?![_\w])/,
    build: (match) => ({ type: 'emphasis', children: parseInline(match[1]) }),
    wordBound: true,
  },
];

export function parseInline(text: string): Inline[] {
  const nodes: Inline[] = [];
  let plain = '';
  let index = 0;
  while (index < text.length) {
    const character = text[index];
    if (character === '\\' && ESCAPABLE.test(text[index + 1] ?? '')) {
      plain += text[index + 1];
      index += 2;
      continue;
    }
    const rest = text.slice(index);
    const afterWord = index > 0 && /\w/.test(text[index - 1]);
    const rule = '`[*_'.includes(character)
      ? INLINE_RULES.map((candidate) => ({ candidate, match: candidate.pattern.exec(rest) })).find(
          ({ candidate, match }) => match && !(candidate.wordBound && afterWord),
        )
      : undefined;
    if (rule?.match) {
      if (plain) nodes.push({ type: 'text', text: plain });
      plain = '';
      nodes.push(rule.candidate.build(rule.match));
      index += rule.match[0].length;
      continue;
    }
    plain += character;
    index += 1;
  }
  if (plain) nodes.push({ type: 'text', text: plain });
  return nodes;
}

const startsBlock = (line: string) => FENCE.test(line) || HEADING.test(line);

const ordered = (marker: string) => /\d/.test(marker);

const draftList = (marker: string): DraftList => ({
  ordered: ordered(marker),
  start: ordered(marker) ? Number.parseInt(marker, 10) : 1,
  items: [],
});

const finishList = (draft: DraftList): ListBlock => ({
  type: 'list',
  ordered: draft.ordered,
  start: draft.start,
  items: draft.items.map((item) => ({
    children: parseInline(item.lines.join('\n')),
    nested: item.nested && finishList(item.nested),
  })),
});

function parseList(lines: string[], start: number, first: RegExpExecArray): [ListBlock, number] {
  const baseIndent = first[1].length;
  const list = draftList(first[2]);
  let index = start;
  while (index < lines.length) {
    const line = lines[index];
    const item = LIST_ITEM.exec(line);
    if (line.trim() === '') {
      const next = lines.slice(index + 1).findIndex((candidate) => candidate.trim() !== '');
      const following = next === -1 ? null : LIST_ITEM.exec(lines[index + 1 + next]);
      if (!following || (following[1].length < baseIndent + 2 && ordered(following[2]) !== list.ordered)) break;
      index += 1 + next;
      continue;
    }
    const current = list.items.at(-1);
    if (item && item[1].length < baseIndent + 2) {
      if (ordered(item[2]) !== list.ordered) break;
      list.items.push({ lines: [item[3]], nested: null });
    } else if (item && current) {
      current.nested ??= draftList(item[2]);
      current.nested.items.push({ lines: [item[3]], nested: null });
    } else if (current && !startsBlock(line)) {
      (current.nested?.items.at(-1) ?? current).lines.push(line.trim());
    } else {
      break;
    }
    index += 1;
  }
  return [finishList(list), index];
}

const inlineText = (nodes: Inline[]): string =>
  nodes.map((node) => (node.type === 'text' || node.type === 'code' ? node.text : inlineText(node.children))).join('');

const blockText = (block: Block): string => {
  switch (block.type) {
    case 'code':
      return block.text;
    case 'list':
      return block.items
        .map((item) => (item.nested ? `${inlineText(item.children)} ${blockText(item.nested)}` : inlineText(item.children)))
        .join(' ');
    default:
      return inlineText(block.children);
  }
};

export function plainText(source: string): string {
  return parseMarkdown(source).map(blockText).join(' ');
}

export function parseMarkdown(source: string): Block[] {
  const lines = source.replace(/\r\n?/g, '\n').split('\n');
  const blocks: Block[] = [];
  let paragraph: string[] = [];
  const flush = () => {
    if (paragraph.length > 0) blocks.push({ type: 'paragraph', children: parseInline(paragraph.join('\n')) });
    paragraph = [];
  };
  let index = 0;
  while (index < lines.length) {
    const line = lines[index];
    const fence = FENCE.exec(line);
    const heading = HEADING.exec(line);
    const item = LIST_ITEM.exec(line);
    if (fence) {
      flush();
      const marker = fence[1];
      let end = index + 1;
      while (end < lines.length) {
        const closing = CLOSING_FENCE.exec(lines[end]);
        if (closing && closing[1][0] === marker[0] && closing[1].length >= marker.length) break;
        end += 1;
      }
      blocks.push({ type: 'code', text: lines.slice(index + 1, end).join('\n') });
      index = end + 1;
    } else if (heading) {
      flush();
      blocks.push({
        type: 'heading',
        level: heading[1].length,
        children: parseInline((heading[2] ?? '').replace(/[ \t]+#+[ \t]*$/, '').trim()),
      });
      index += 1;
    } else if (item && item[1].length <= 3) {
      flush();
      const [list, next] = parseList(lines, index, item);
      blocks.push(list);
      index = next;
    } else if (line.trim() === '') {
      flush();
      index += 1;
    } else {
      paragraph.push(line.trim());
      index += 1;
    }
  }
  flush();
  return blocks;
}
