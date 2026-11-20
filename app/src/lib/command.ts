export type TokenKind = 'command' | 'word' | 'flag' | 'string' | 'operator' | 'variable' | 'comment' | 'space';

export interface Token {
  kind: TokenKind;
  text: string;
}

const SPACE = /\s+/y;
const COMMENT = /#[^\n]*/y;
const SINGLE_QUOTED = /'[^']*'?/y;
const DOUBLE_QUOTED = /"(?:[^"\\`]|[\\`][\s\S]?)*"?/y;
const OPERATOR = /&&|\|\||>>|>&|<&|[|;<>&]/y;
const VARIABLE = /\$(?:\{[^}]*\}?|env:\w+|\w+|[?$!#@*])/iy;
const WORD = /[^\s'"|;&<>$]+|\$/y;
export const REDIRECTS = new Set(['>', '>>', '<', '>&', '<&']);

function nextToken(source: string, index: number, wordStart: boolean, commandPosition: boolean): Token {
  const at = (pattern: RegExp) => {
    pattern.lastIndex = index;
    return pattern.exec(source)?.[0] ?? null;
  };
  const space = at(SPACE);
  if (space) return { kind: 'space', text: space };
  const comment = wordStart ? at(COMMENT) : null;
  if (comment) return { kind: 'comment', text: comment };
  if (source[index] === "'" || source[index] === '"') {
    return { kind: 'string', text: at(source[index] === "'" ? SINGLE_QUOTED : DOUBLE_QUOTED) ?? source[index] };
  }
  const operator = at(OPERATOR);
  if (operator) return { kind: 'operator', text: operator };
  const variable = at(VARIABLE);
  if (variable) return { kind: 'variable', text: variable };
  const word = at(WORD) ?? source[index];
  if (!wordStart) return { kind: 'word', text: word };
  if (commandPosition) return { kind: 'command', text: word };
  return { kind: word.length > 1 && word.startsWith('-') ? 'flag' : 'word', text: word };
}

export function tokenizeCommand(source: string): Token[] {
  const tokens: Token[] = [];
  let index = 0;
  let expectCommand = true;
  let lastSignificant: Token | undefined;
  while (index < source.length) {
    const previous = tokens.at(-1);
    const wordStart = !previous || previous.kind === 'space' || previous.kind === 'operator';
    const redirectTarget = wordStart && lastSignificant?.kind === 'operator' && REDIRECTS.has(lastSignificant.text);
    const token = nextToken(source, index, wordStart, expectCommand && !redirectTarget);
    if (token.kind === 'space') {
      const continued = previous !== undefined && /[\\`]$/.test(previous.text);
      if (token.text.includes('\n') && !continued) expectCommand = true;
    } else if (token.kind === 'operator') {
      if (!REDIRECTS.has(token.text)) expectCommand = true;
    } else if (token.kind !== 'comment' && wordStart && !redirectTarget) {
      expectCommand = false;
    }
    if (token.kind !== 'space') lastSignificant = token;
    tokens.push(token);
    index += token.text.length;
  }
  return tokens;
}
