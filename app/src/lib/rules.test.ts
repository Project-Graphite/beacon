import assert from 'node:assert/strict';
import { test } from 'node:test';
import { addRules, formatRule, parseRule, suggestedRule } from './rules.ts';

test('rules parse as a tool alone or a tool with a specifier', () => {
  assert.deepEqual(parseRule('Bash'), { toolName: 'Bash' });
  assert.deepEqual(parseRule('Bash(node -e *)'), { toolName: 'Bash', ruleContent: 'node -e *' });
  assert.deepEqual(parseRule('Bash(node -e "console.log(\'x\')")'), {
    toolName: 'Bash',
    ruleContent: 'node -e "console.log(\'x\')"',
  });
  assert.deepEqual(parseRule('mcp__github__get_issue'), { toolName: 'mcp__github__get_issue' });
  assert.deepEqual(parseRule('Edit(src/**)'), { toolName: 'Edit', ruleContent: 'src/**' });
  for (const text of ['', 'Bash(', 'Bash()', '(npm test)', 'Bash(x) extra', 'Bash(x)y', 'Read File', ' Bash', 'Ba)sh']) {
    assert.equal(parseRule(text), null, text);
  }
});

test('formatting a parsed rule gives the text back', () => {
  for (const text of ['Bash', 'Bash(npm run *)', 'WebFetch(domain:example.test)']) {
    assert.equal(formatRule(parseRule(text)!), text);
  }
});

test('the editor starts from the first addRules suggestion or the tool alone', () => {
  const suggestions = [
    { type: 'setMode', mode: 'acceptEdits', destination: 'session' },
    {
      type: 'addRules',
      rules: [{ toolName: 'Bash', ruleContent: 'node -e "console.log(\'beacon-spike-ok\')"' }],
      behavior: 'allow',
      destination: 'localSettings',
    },
    { type: 'addRules', rules: [{ toolName: 'Read' }], behavior: 'allow', destination: 'userSettings' },
  ];
  assert.deepEqual(suggestedRule('Bash', suggestions), {
    text: 'Bash(node -e "console.log(\'beacon-spike-ok\')")',
    destination: 'localSettings',
  });
  assert.deepEqual(suggestedRule('Read', [{ type: 'addRules', rules: [{ toolName: 'Read' }], destination: 'cloud' }]), {
    text: 'Read',
    destination: 'session',
  });
  assert.deepEqual(suggestedRule('WebFetch', null), { text: 'WebFetch', destination: 'session' });
  assert.deepEqual(suggestedRule('Edit', [{ type: 'addRules', rules: [] }]), { text: 'Edit', destination: 'session' });
});

test('a saved rule is one addRules entry that allows', () => {
  assert.deepEqual(addRules({ toolName: 'Bash', ruleContent: 'node -e *' }, 'session'), {
    type: 'addRules',
    rules: [{ toolName: 'Bash', ruleContent: 'node -e *' }],
    behavior: 'allow',
    destination: 'session',
  });
  assert.deepEqual(JSON.stringify(addRules({ toolName: 'WebFetch' }, 'userSettings')),
    '{"type":"addRules","rules":[{"toolName":"WebFetch"}],"behavior":"allow","destination":"userSettings"}');
});
