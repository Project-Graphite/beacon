const MASK = '***';
const SECRET_NAME_PARTS = ['KEY', 'TOKEN', 'SECRET', 'PASSWORD', 'PASSWD'];
const TOKEN_PREFIXES = [
  'sk-',
  'sk_live_',
  'rk_live_',
  'ghp_',
  'gho_',
  'ghu_',
  'ghs_',
  'ghr_',
  'github_pat_',
  'glpat-',
  'npm_',
  'xox',
  'AKIA',
  'AIza',
];
const MIN_PREFIXED_TOKEN_LEN = 20;
const JWT_START = 'eyJ';
const PEM_START = '-----BEGIN ';

type Span = [keptEnd: number, end: number];

const isNameChar = (character: string) => /^[A-Za-z0-9_.-]$/.test(character);
const isQuote = (character: string | undefined) => character === '"' || character === "'";
const isSpace = (character: string) => /^\s$/.test(character);
const isBlank = (character: string) => character === ' ' || character === '\t';
const isTokenChar = (character: string) => /^[A-Za-z0-9_-]$/.test(character);

function runEnd(text: string, start: number, keep: (character: string) => boolean): number {
  let index = start;
  while (index < text.length && keep(text[index])) index++;
  return index;
}

function isSecretName(name: string): boolean {
  const upper = name.toUpperCase();
  return SECRET_NAME_PARTS.some((part) => upper.includes(part));
}

function headerValue(text: string, start: number, label: string, endsValue: (character: string) => boolean): Span | null {
  if (text.slice(start, start + label.length).toLowerCase() !== label) return null;
  const keptEnd = runEnd(text, start + label.length, isBlank);
  const end = runEnd(text, keptEnd, (character) => !endsValue(character));
  return end > keptEnd ? [keptEnd, end] : null;
}

function bearerToken(text: string, start: number): Span | null {
  const span = headerValue(text, start, 'bearer', (character) => isSpace(character) || isQuote(character));
  return span && span[0] > start + 'bearer'.length ? span : null;
}

function prefixedToken(text: string, start: number): Span | null {
  if (!TOKEN_PREFIXES.some((prefix) => text.startsWith(prefix, start))) return null;
  const end = runEnd(text, start, isTokenChar);
  return end - start >= MIN_PREFIXED_TOKEN_LEN ? [start, end] : null;
}

function jsonWebToken(text: string, start: number): Span | null {
  if (!text.startsWith(JWT_START, start)) return null;
  const end = runEnd(text, start, (character) => isTokenChar(character) || character === '.');
  const token = text.slice(start, end);
  return token.length >= MIN_PREFIXED_TOKEN_LEN && token.split('.').length >= 3 ? [start, end] : null;
}

function privateKeyBlock(text: string, start: number): Span | null {
  if (!text.startsWith(PEM_START, start)) return null;
  const headerEnd = text.indexOf('-----', start + PEM_START.length);
  if (headerEnd === -1 || !text.slice(start, headerEnd).endsWith('PRIVATE KEY')) return null;
  const keptEnd = headerEnd + 5;
  const footer = text.indexOf('-----END ', keptEnd);
  const footerEnd = footer === -1 ? -1 : text.indexOf('-----', footer + 5);
  const end = footerEnd === -1 ? text.length : footerEnd + 5;
  return end > keptEnd ? [keptEnd, end] : null;
}

function urlPassword(text: string, start: number): Span | null {
  const schemeEnd = runEnd(text, start, (character) => /^[A-Za-z0-9+.-]$/.test(character));
  if (schemeEnd === start || !text.startsWith('://', schemeEnd)) return null;
  const userEnd = runEnd(text, schemeEnd + 3, (character) => !/^[\s/@:?#"']$/.test(character));
  if (text[userEnd] !== ':') return null;
  const end = runEnd(text, userEnd + 1, (character) => !/^[\s/@?#"']$/.test(character));
  return end > userEnd + 1 && text[end] === '@' ? [userEnd + 1, end] : null;
}

function secretAssignment(text: string, start: number): Span | null {
  const nameEnd = runEnd(text, start, isNameChar);
  if (!isSecretName(text.slice(start, nameEnd))) return null;
  const quotedName = isQuote(text[nameEnd]);
  const operator = runEnd(text, quotedName ? nameEnd + 1 : nameEnd, isBlank);
  const valueStart = runEnd(text, operator + 1, isBlank);
  const keptEnd =
    text[operator] === '=' || (text[operator] === ':' && (quotedName || isQuote(text[valueStart])))
      ? valueStart
      : text[start] === '-' && operator > nameEnd && text[operator] !== '-'
        ? operator
        : null;
  if (keptEnd === null) return null;
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
    privateKeyBlock(text, start) ??
    urlPassword(text, start) ??
    prefixedToken(text, start) ??
    jsonWebToken(text, start) ??
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

export function redactValue(name: string, value: string): string {
  return isSecretName(name) ? MASK : redact(value);
}

export function redactJson(name: string, value: unknown): unknown {
  if (typeof value === 'string') return redactValue(name, value);
  if (isSecretName(name)) return MASK;
  if (Array.isArray(value)) return value.map((item) => redactJson(name, item));
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, redactJson(key, item)]));
  }
  return value;
}
