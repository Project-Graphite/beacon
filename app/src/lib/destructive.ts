import { REDIRECTS, tokenizeCommand, type Token } from './command.ts';
import { relativeInside } from './paths.ts';

const PREFIXES = new Set(['sudo', 'doas', 'command', 'nohup', 'time', 'exec', 'env', 'nice', 'xargs']);
const MENTIONS = new Set([
  'echo',
  'printf',
  'print',
  'write-host',
  'write-output',
  'write-error',
  'write-warning',
  'write-verbose',
  'grep',
  'rg',
  'findstr',
  'select-string',
]);
const SCRIPT_FLAGS: Record<string, string[]> = {
  bash: ['-c'],
  sh: ['-c'],
  zsh: ['-c'],
  dash: ['-c'],
  pwsh: ['-c', '-command'],
  powershell: ['-c', '-command'],
  cmd: ['/c', '/k'],
};
const DOWNLOADERS = new Set(['curl', 'wget']);
const SCRIPT_RUNNERS = new Set(['sh', 'bash', 'zsh', 'dash', 'pwsh', 'powershell', 'iex', 'invoke-expression']);
const RM_FLAG_LETTERS = /^-[rRfdivI]+$/;
const WRITING_TOOLS = ['Edit', 'Write', 'NotebookEdit'];

const unquoted = (token: Token) =>
  token.kind === 'string' ? token.text.slice(1, token.text.length > 1 && token.text.endsWith(token.text[0]) ? -1 : undefined) : token.text;

function pipelines(source: string): string[][][] {
  const found: string[][][] = [[[]]];
  const command = () => found[found.length - 1][found[found.length - 1].length - 1];
  let word: string | null = null;
  const endWord = () => {
    if (word !== null) command().push(word);
    word = null;
  };
  for (const token of tokenizeCommand(source)) {
    if (token.kind === 'space' || token.kind === 'comment' || token.kind === 'operator') endWord();
    if (token.kind === 'space' && token.text.includes('\n')) {
      const words = command();
      const last = words.at(-1);
      if (last !== undefined && /[\\`]$/.test(last)) {
        words[words.length - 1] = last.slice(0, -1);
      } else {
        found.push([[]]);
      }
    } else if (token.kind === 'operator') {
      if (token.text === '|') found[found.length - 1].push([]);
      else if (!REDIRECTS.has(token.text)) found.push([[]]);
    } else if (token.kind !== 'space' && token.kind !== 'comment') {
      word = (word ?? '') + unquoted(token);
    }
  }
  endWord();
  return found
    .map((pipeline) => pipeline.map(withoutPrefixes).filter((words) => words.length > 0))
    .filter((pipeline) => pipeline.length > 0);
}

function withoutPrefixes(words: string[]): string[] {
  const start = words.findIndex((word) => !PREFIXES.has(commandName(word)) && !/^\w+=/.test(word));
  return start === -1 ? [] : words.slice(start);
}

function commandName(word: string): string {
  return (word.split(/[\\/]/).pop() ?? word).toLowerCase().replace(/\.exe$/, '');
}

const shortFlags = (args: string[], letter: string) => args.some((arg) => /^-[a-zA-Z]+$/.test(arg) && arg.includes(letter));

function recurseParameter(arg: string, minimumLength: number): boolean {
  const name = arg.split(':')[0].toLowerCase();
  return name.length >= minimumLength && name.startsWith('-r') && 'recurse'.startsWith(name.slice(1));
}

function rmReason(args: string[]): string | null {
  const options = args.includes('--') ? args.slice(0, args.indexOf('--')) : args;
  const groups = options.filter((arg) => RM_FLAG_LETTERS.test(arg));
  const recursive = options.includes('--recursive') || groups.some((group) => /[rR]/.test(group));
  const force = options.includes('--force') || groups.some((group) => group.includes('f'));
  if (recursive && force) return 'rm with recursive and force flags';
  if (args.some((arg) => recurseParameter(arg, 4) || arg.toLowerCase() === '/s')) return 'recursive delete';
  return null;
}

function gitReason(args: string[]): string | null {
  let index = 0;
  while (index < args.length && args[index].startsWith('-')) {
    index += ['-C', '-c', '--git-dir', '--work-tree'].includes(args[index]) ? 2 : 1;
  }
  const [subcommand, ...rest] = args.slice(index);
  switch (subcommand) {
    case 'push':
      return rest.some((arg) => arg === '--force' || arg.startsWith('--force-with-lease')) || shortFlags(rest, 'f')
        ? 'git push --force'
        : null;
    case 'reset':
      return rest.includes('--hard') ? 'git reset --hard' : null;
    case 'clean':
      return rest.includes('--force') || shortFlags(rest, 'f') ? 'git clean -f' : null;
    case 'checkout':
    case 'restore':
      return rest.includes('.') ? `git ${subcommand} . discards local changes` : null;
    default:
      return null;
  }
}

function sqlReason(text: string): string | null {
  const dropped = /\bdrop\s+(table|database|schema)\b/i.exec(text);
  if (dropped) return `DROP ${dropped[1].toUpperCase()}`;
  if (/\btruncate\s+(?:table\s+)?[\w"`[]/i.test(text)) return 'TRUNCATE';
  const unbounded = text
    .split(';')
    .some((statement) => /\bdelete\s+from\b/i.test(statement) && !/\bwhere\b/i.test(statement));
  return unbounded ? 'DELETE FROM without WHERE' : null;
}

function commandReason(words: string[]): string | null {
  const [first, ...args] = words;
  const name = commandName(first);
  if (MENTIONS.has(name)) return null;
  const scriptFlag = args.findIndex((arg) => SCRIPT_FLAGS[name]?.includes(arg.toLowerCase()));
  if (scriptFlag !== -1) return scriptReason(args.slice(scriptFlag + 1).join(' '));
  switch (name) {
    case 'rm':
      return rmReason(args);
    case 'remove-item':
    case 'ri':
      return args.some((arg) => recurseParameter(arg, 2)) ? 'recursive Remove-Item' : null;
    case 'del':
    case 'erase':
    case 'rd':
    case 'rmdir':
      return args.some((arg) => arg.toLowerCase() === '/s' || recurseParameter(arg, 2)) ? 'recursive delete' : null;
    case 'git':
      return gitReason(args);
    case 'dd':
      return args.some((arg) => arg.startsWith('of=')) ? 'dd writing with of=' : null;
    case 'format':
      return args.some((arg) => /^[a-z]:$/i.test(arg)) ? 'format of a drive' : null;
    case 'shutdown':
      return 'shutdown';
    case 'chmod':
      return (args.includes('--recursive') || shortFlags(args, 'R')) && args.includes('777') ? 'chmod -R 777' : null;
  }
  if (name === 'mkfs' || name.startsWith('mkfs.')) return 'mkfs';
  return sqlReason(args.join(' '));
}

function scriptReason(script: string): string | null {
  for (const pipeline of pipelines(script)) {
    const names = pipeline.map((words) => commandName(words[0]));
    const download = names.findIndex((name) => DOWNLOADERS.has(name));
    const runner = names.slice(download + 1).find((name) => SCRIPT_RUNNERS.has(name));
    if (download !== -1 && runner) return `${names[download]} piped into ${runner}`;
    for (const words of pipeline) {
      const reason = commandReason(words);
      if (reason) return reason;
    }
  }
  return null;
}

export function destructiveReason(toolName: string, toolInput: unknown, cwd: string | null): string | null {
  const input = (toolInput ?? {}) as Record<string, unknown>;
  if ((toolName === 'Bash' || toolName === 'PowerShell') && typeof input.command === 'string') {
    return scriptReason(input.command);
  }
  const path = input.file_path ?? input.notebook_path;
  if (WRITING_TOOLS.includes(toolName) && typeof path === 'string' && cwd !== null && relativeInside(path, cwd) === null) {
    return 'writes outside the working directory';
  }
  return null;
}
