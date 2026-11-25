import { REDIRECTS, tokenizeCommand, type Token } from './command.ts';
import { relativeInside } from './paths.ts';

const PREFIXES = new Map<string, string[]>([
  ['sudo', ['-u', '-g', '-h', '-p', '-C', '-r', '-t', '-U', '-D', '-R']],
  ['doas', ['-u', '-C']],
  ['command', []],
  ['nohup', []],
  ['time', ['-f', '-o']],
  ['exec', ['-a']],
  ['env', ['-u', '-C', '-S']],
  ['nice', ['-n']],
  ['timeout', ['-s', '-k']],
  ['xargs', ['-I', '-n', '-P', '-d', '-L', '-E', '-s', '-a', '-l']],
  ['npx', ['-p', '--package']],
]);
const BLOCK_KEYWORDS = new Set(['then', 'do', 'else', 'elif']);
const FIND_EXEC = new Set(['-exec', '-execdir', '-ok', '-okdir']);
const GROUP = /[({`]([^(){}`]*)[)}`]/g;
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
const POSIX_SCRIPT_FLAG = /^-[a-z]*c[a-z]*$/;
const POWERSHELL_SCRIPT_FLAG = /^-c(?:ommand)?$/i;
const SCRIPT_FLAGS: Record<string, RegExp> = {
  bash: POSIX_SCRIPT_FLAG,
  sh: POSIX_SCRIPT_FLAG,
  zsh: POSIX_SCRIPT_FLAG,
  dash: POSIX_SCRIPT_FLAG,
  pwsh: POWERSHELL_SCRIPT_FLAG,
  powershell: POWERSHELL_SCRIPT_FLAG,
  cmd: /^\/[ck]$/i,
};
const DOWNLOADERS = new Set([
  'curl',
  'wget',
  'iwr',
  'irm',
  'invoke-webrequest',
  'invoke-restmethod',
  'start-bitstransfer',
]);
const SCRIPT_RUNNERS = new Set(['sh', 'bash', 'zsh', 'dash', 'pwsh', 'powershell', 'iex', 'invoke-expression', 'eval']);
const INTERPRETERS = new Set(['python', 'python3', 'node', 'perl', 'ruby']);
const DOTNET_DOWNLOAD = /webclient|download(?:string|file)/i;
const RUN_DOWNLOAD = new RegExp(
  `\\b(${[...SCRIPT_RUNNERS].join('|')})\\b[^|;&\\n]*[(\`][^|;&\\n]*\\b(?:${[...DOWNLOADERS].join('|')}|${DOTNET_DOWNLOAD.source})`,
  'i',
);
const RM_FLAG_LETTERS = /^-[rRfdivI]+$/;
const WRITING_TOOLS = ['Edit', 'Write', 'NotebookEdit'];

const unquoted = (token: Token) =>
  token.kind === 'string' ? token.text.slice(1, token.text.length > 1 && token.text.endsWith(token.text[0]) ? -1 : undefined) : token.text;

function pipelines(source: string): string[][][] {
  const found: string[][][] = [[[]]];
  const command = () => found[found.length - 1][found[found.length - 1].length - 1];
  let word: string | null = null;
  const endWord = () => {
    if (word === null) return;
    const atStart = command().length === 0;
    const text = atStart ? word.replace(/^(?:\$?[({]|`)+/, '') : word;
    if (text !== '' && !(atStart && BLOCK_KEYWORDS.has(text))) command().push(text);
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
  let index = 0;
  while (index < words.length) {
    const name = commandName(words[index]);
    const valued = PREFIXES.get(name);
    if (valued === undefined && !/^\w+=/.test(words[index])) break;
    index++;
    if (valued === undefined) continue;
    while (index < words.length && words[index].startsWith('-')) index += valued.includes(words[index]) ? 2 : 1;
    if (name === 'timeout') index++;
  }
  return words.slice(index);
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
  if (recursive) return force ? 'rm with recursive and force flags' : 'rm with recursive flag';
  if (args.some((arg) => recurseParameter(arg, 3) || arg.toLowerCase() === '/s')) return 'recursive delete';
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
      if (rest.some((arg) => arg === '--force' || arg.startsWith('--force-with-lease') || /^\+./.test(arg)) || shortFlags(rest, 'f')) {
        return 'git push --force';
      }
      if (rest.includes('--delete') || shortFlags(rest, 'd') || rest.some((arg) => /^:./.test(arg))) return 'git push --delete';
      return rest.includes('--mirror') ? 'git push --mirror' : null;
    case 'reset':
      return rest.includes('--hard') ? 'git reset --hard' : null;
    case 'clean':
      return rest.includes('--force') || shortFlags(rest, 'f') ? 'git clean -f' : null;
    case 'branch':
      return shortFlags(rest, 'D') ? 'git branch -D' : null;
    case 'stash':
      return rest[0] === 'clear' ? 'git stash clear' : null;
    case 'checkout':
    case 'restore':
      if (subcommand === 'checkout' && (rest.includes('--force') || shortFlags(rest, 'f'))) return 'git checkout -f';
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
  const scriptFlag = args.findIndex((arg) => SCRIPT_FLAGS[name]?.test(arg));
  if (scriptFlag !== -1) return scriptReason(args.slice(scriptFlag + 1).join(' '));
  switch (name) {
    case 'eval':
    case 'iex':
    case 'invoke-expression':
      return scriptReason(args.join(' '));
    case 'find': {
      if (args.includes('-delete')) return 'find -delete';
      const exec = args.findIndex((arg) => FIND_EXEC.has(arg));
      const command = exec === -1 ? [] : withoutPrefixes(args.slice(exec + 1));
      return command.length > 0 ? commandReason(command) : null;
    }
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
    case 'format-volume':
      return 'Format-Volume';
    case 'clear-disk':
      return 'Clear-Disk';
    case 'rimraf':
      return 'rimraf';
    case 'shutdown':
      return 'shutdown';
    case 'chmod':
      return (args.includes('--recursive') || shortFlags(args, 'R')) && args.includes('777') ? 'chmod -R 777' : null;
  }
  if (name === 'mkfs' || name.startsWith('mkfs.')) return 'mkfs';
  return sqlReason(args.join(' '));
}

const runsInput = (words: string[]) => {
  const name = commandName(words[0]);
  return SCRIPT_RUNNERS.has(name) || (INTERPRETERS.has(name) && (words.length === 1 || words[1] === '-'));
};

function groupBodies(script: string): string[] {
  const bodies: string[] = [];
  for (let rest = script, previous = ''; previous !== rest; ) {
    previous = rest;
    rest = rest.replace(GROUP, (_, body: string) => {
      bodies.push(body);
      return ' ; ';
    });
  }
  return bodies;
}

function pipelineReason(script: string): string | null {
  for (const pipeline of pipelines(script)) {
    const download = pipeline.findIndex(
      (words) => DOWNLOADERS.has(commandName(words[0])) || DOTNET_DOWNLOAD.test(words.join(' ')),
    );
    const runner = download === -1 ? undefined : pipeline.slice(download + 1).find(runsInput);
    if (runner) return `${commandName(pipeline[download][0])} piped into ${commandName(runner[0])}`;
    for (const words of pipeline) {
      const reason = commandReason(words);
      if (reason) return reason;
    }
  }
  return null;
}

function scriptReason(script: string): string | null {
  const runningDownload = RUN_DOWNLOAD.exec(script);
  if (runningDownload) return `${runningDownload[1].toLowerCase()} running a download`;
  for (const part of [script, ...groupBodies(script)]) {
    const reason = pipelineReason(part);
    if (reason) return reason;
  }
  return null;
}

export function destructiveReason(toolName: string, toolInput: unknown, cwd: string | null): string | null {
  const input = (toolInput ?? {}) as Record<string, unknown>;
  if ((toolName === 'Bash' || toolName === 'PowerShell') && typeof input.command === 'string') {
    return scriptReason(input.command);
  }
  if (toolName.startsWith('mcp__')) {
    for (const script of [input.command, input.cmd, input.script]) {
      const reason = typeof script === 'string' ? scriptReason(script) : null;
      if (reason) return reason;
    }
  }
  const path = input.file_path ?? input.notebook_path;
  if (WRITING_TOOLS.includes(toolName) && typeof path === 'string' && cwd !== null && relativeInside(path, cwd) === null) {
    return 'writes outside the working directory';
  }
  return null;
}
