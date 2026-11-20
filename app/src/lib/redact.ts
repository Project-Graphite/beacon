const MASK = '***';
const SECRET_NAME_PARTS = ['KEY', 'TOKEN', 'SECRET', 'PASSWORD', 'PASSWD'];
const TOKEN_PREFIXES = ['sk-', 'ghp_', 'xox'];
const MIN_PREFIXED_TOKEN_LEN = 20;

type Span = [keptEnd: number, end: number];

const isNameChar = (character: string) => /^[A-Za-z0-9_.-]$/.test(character);
const isQuote = (character: string | undefined) => character === '"' || character === "'";
const isSpace = (character: string) => /^\s$/.test(character);

function runEnd(text: string, start: number, keep: (character: string) => boolean): number {
  let index = start;
  while (index < text.length && keep(text[index])) index++;
  return index;
}

function headerValue(text: string, start: number, label: string, endsValue: (character: string) => boolean): Span | null {
  if (text.slice(start, start + label.length).toLowerCase() !== label) return null;
  const keptEnd = runEnd(text, start + label.length, (character) => character === ' ' || character === '\t');
  const end = runEnd(text, keptEnd, (character) => !endsValue(character));
  return end > keptEnd ? [keptEnd, end] : null;
}

function bearerToken(text: string, start: number): Span | null {
  const span = headerValue(text, start, 'bearer', (character) => isSpace(character) || isQuote(character));
  return span && span[0] > start + 'bearer'.length ? span : null;
}

function prefixedToken(text: string, start: number): Span | null {
  if (!TOKEN_PREFIXES.some((prefix) => text.startsWith(prefix, start))) return null;
  const end = runEnd(text, start, (character) => /^[A-Za-z0-9_-]$/.test(character));
  return end - start >= MIN_PREFIXED_TOKEN_LEN ? [start, end] : null;
}

function secretAssignment(text: string, start: number): Span | null {
  const nameEnd = runEnd(text, start, isNameChar);
  const name = text.slice(start, nameEnd).toUpperCase();
  if (text[nameEnd] !== '=' || !SECRET_NAME_PARTS.some((part) => name.includes(part))) return null;
  const keptEnd = nameEnd + 1;
  const quote = text[keptEnd];
  const closing = isQuote(quote) ? text.indexOf(quote, keptEnd + 1) : -1;
  const end = !isQuote(quote)
    ? runEnd(text, keptEnd, (character) => !isSpace(character) && !isQuote(character))
    : closing === -1
      ? text.length
      : closing + 1;
  return end > keptEnd ? [keptEnd, end] : null;
}

function secretAt(text: string, start: number): Span | null {
  return (
    headerValue(text, start, 'authorization:', (character) => ['"', "'", '\r', '\n'].includes(character)) ??
    bearerToken(text, start) ??
    prefixedToken(text, start) ??
    secretAssignment(text, start)
  );
}

export function redact(text: string): string {
  let redacted = '';
  let index = 0;
  let atWordStart = true;
  while (index < text.length) {
    const secret = atWordStart ? secretAt(text, index) : null;
    if (secret) {
      redacted += text.slice(index, secret[0]) + MASK;
      index = secret[1];
      atWordStart = false;
      continue;
    }
    redacted += text[index];
    atWordStart = !isNameChar(text[index]);
    index++;
  }
  return redacted;
}
