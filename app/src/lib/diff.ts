export type DiffKind = 'context' | 'removed' | 'added';

export interface DiffLine {
  kind: DiffKind;
  text: string;
}

const MAX_TABLE_CELLS = 1_000_000;
const WRITE_PREVIEW_LINES = 20;

function linesOf(text: string): string[] {
  const body = text.replace(/\r\n/g, '\n').replace(/\n$/, '');
  return body === '' ? [] : body.split('\n');
}

export function lineDiff(before: string, after: string): DiffLine[] {
  const old = linesOf(before);
  const updated = linesOf(after);
  let prefix = 0;
  while (prefix < old.length && prefix < updated.length && old[prefix] === updated[prefix]) prefix++;
  let suffix = 0;
  while (
    suffix < old.length - prefix &&
    suffix < updated.length - prefix &&
    old[old.length - 1 - suffix] === updated[updated.length - 1 - suffix]
  ) {
    suffix++;
  }
  const removed = old.slice(prefix, old.length - suffix);
  const added = updated.slice(prefix, updated.length - suffix);
  const context = (lines: string[]) => lines.map((text): DiffLine => ({ kind: 'context', text }));
  return [
    ...context(old.slice(0, prefix)),
    ...middleDiff(removed, added),
    ...context(old.slice(old.length - suffix)),
  ];
}

function middleDiff(removed: string[], added: string[]): DiffLine[] {
  const rows = removed.length + 1;
  const columns = added.length + 1;
  if (removed.length === 0 || added.length === 0 || rows * columns > MAX_TABLE_CELLS) {
    return [
      ...removed.map((text): DiffLine => ({ kind: 'removed', text })),
      ...added.map((text): DiffLine => ({ kind: 'added', text })),
    ];
  }
  const common = new Uint32Array(rows * columns);
  for (let row = removed.length - 1; row >= 0; row--) {
    for (let column = added.length - 1; column >= 0; column--) {
      common[row * columns + column] =
        removed[row] === added[column]
          ? common[(row + 1) * columns + column + 1] + 1
          : Math.max(common[(row + 1) * columns + column], common[row * columns + column + 1]);
    }
  }
  const lines: DiffLine[] = [];
  let row = 0;
  let column = 0;
  while (row < removed.length || column < added.length) {
    if (row < removed.length && column < added.length && removed[row] === added[column]) {
      lines.push({ kind: 'context', text: removed[row] });
      row++;
      column++;
    } else if (
      column === added.length ||
      (row < removed.length && common[(row + 1) * columns + column] >= common[row * columns + column + 1])
    ) {
      lines.push({ kind: 'removed', text: removed[row] });
      row++;
    } else {
      lines.push({ kind: 'added', text: added[column] });
      column++;
    }
  }
  return lines;
}

export function writePreview(content: string): { lines: DiffLine[]; more: number } {
  const lines = linesOf(content);
  return {
    lines: lines.slice(0, WRITE_PREVIEW_LINES).map((text) => ({ kind: 'added', text })),
    more: Math.max(0, lines.length - WRITE_PREVIEW_LINES),
  };
}
