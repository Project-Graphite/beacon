export interface KeyPress {
  key: string;
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

const MODIFIER_KEYS = new Set(['Control', 'Alt', 'Shift', 'Meta', 'AltGraph', 'OS']);

const NAMED_CODES: Record<string, string> = {
  Space: 'Space',
  Enter: 'Enter',
  Tab: 'Tab',
  Backquote: 'Backquote',
  Minus: 'Minus',
  Equal: 'Equal',
  BracketLeft: 'BracketLeft',
  BracketRight: 'BracketRight',
  Backslash: 'Backslash',
  Semicolon: 'Semicolon',
  Quote: 'Quote',
  Comma: 'Comma',
  Period: 'Period',
  Slash: 'Slash',
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  Insert: 'Insert',
  Delete: 'Delete',
};

function keyName(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1\d|2[0-4])$/.test(code)) return code;
  if (/^Numpad\d$/.test(code)) return `Num${code.slice(6)}`;
  return NAMED_CODES[code] ?? null;
}

export type Recorded = { kind: 'shortcut'; text: string } | { kind: 'cancel' } | { kind: 'pending' } | { kind: 'invalid'; reason: string };

export function recordShortcut(press: KeyPress): Recorded {
  const modifiers = [press.ctrlKey && 'Ctrl', press.altKey && 'Alt', press.shiftKey && 'Shift', press.metaKey && 'Super'].filter(
    (modifier): modifier is string => Boolean(modifier),
  );
  if (press.key === 'Escape' && modifiers.length === 0) return { kind: 'cancel' };
  if (MODIFIER_KEYS.has(press.key)) return { kind: 'pending' };
  const key = keyName(press.code);
  if (key === null) return { kind: 'invalid', reason: 'That key cannot be used in a shortcut' };
  const functionKey = /^F\d+$/.test(key);
  const anchored = press.ctrlKey || press.altKey || press.metaKey;
  if (!anchored || (!functionKey && modifiers.length < 2)) {
    return { kind: 'invalid', reason: functionKey ? 'Hold Ctrl, Alt or Win with the key' : 'Hold two of Ctrl, Alt, Shift or Win with the key' };
  }
  return { kind: 'shortcut', text: [...modifiers, key].join('+') };
}

const KEY_LABELS: Record<string, string> = {
  Super: 'Win',
  Backquote: '`',
  Minus: '-',
  Equal: '=',
  BracketLeft: '[',
  BracketRight: ']',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  Comma: ',',
  Period: '.',
  Slash: '/',
  Up: '↑',
  Down: '↓',
  Left: '←',
  Right: '→',
};

export const shortcutKeys = (shortcut: string) =>
  shortcut
    .split('+')
    .filter(Boolean)
    .map((key) => KEY_LABELS[key] ?? key);
