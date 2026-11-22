import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { PendingDecision } from '../beacon';
import { argumentEntries, decisionMark, mcpTool, requestLine, requestMeta, splitUrl } from './requests.ts';

test('allowing or answering shows a check, any deny shows a cross', () => {
  assert.equal(decisionMark({ type: 'allow' }), 'check');
  assert.equal(decisionMark({ type: 'allow_with_input', input: {} }), 'check');
  assert.equal(decisionMark({ type: 'allow_always', permissions: [] }), 'check');
  assert.equal(decisionMark({ type: 'answer', answers: {} }), 'check');
  assert.equal(decisionMark({ type: 'deny' }), 'cross');
  assert.equal(decisionMark({ type: 'deny_and_stop', message: 'stop' }), 'cross');
});

const NOW = 1_795_100_000_000;

const request = (fields: Partial<PendingDecision>): PendingDecision => ({
  id: 1,
  session_id: 's',
  agent_id: null,
  tool_name: 'Bash',
  tool_input: {},
  permission_suggestions: null,
  kind: 'tool',
  created_ms: NOW - 12_000,
  tool_use_id: null,
  preview: '',
  cwd: 'C:\\work',
  permission_mode: 'default',
  agent_type: null,
  expires_ms: NOW - 12_000 + 3_600_000,
  plan: null,
  ...fields,
});

test('a compact request line reads as the command, the first question or the plan', () => {
  assert.equal(requestLine(request({ preview: 'npm test' })), 'npm test');
  const ask = (questions: unknown[]) =>
    requestLine(request({ kind: 'question', tool_name: 'AskUserQuestion', tool_input: { questions }, preview: '{"questions":[]}' }));
  assert.equal(ask([{ question: 'Which database?' }]), 'Which database?');
  assert.equal(ask([{ question: 'Which database?' }, { question: 'Which checks?' }]), 'Which database? (+1 more)');
  assert.equal(ask([]), 'Question');
  assert.equal(requestLine(request({ kind: 'plan', tool_name: 'ExitPlanMode', preview: '{"plan":"# Plan"}' })), 'Plan ready for review');
});

test('the meta line names the folder, mode, subagent, age and a close expiry', () => {
  assert.deepEqual(requestMeta(request({}), NOW), ['C:\\work', '12s']);
  assert.deepEqual(
    requestMeta(request({ permission_mode: 'acceptEdits', agent_id: 'a1', agent_type: 'Explore', expires_ms: NOW + 11 * 60_000 + 5 }), NOW),
    ['C:\\work', 'acceptEdits', 'from subagent Explore', '12s', 'expires in 12m'],
  );
  assert.deepEqual(requestMeta(request({ agent_id: 'a1', cwd: null, expires_ms: NOW + 15 * 60_000 }), NOW), [
    'from subagent',
    '12s',
  ]);
  assert.deepEqual(requestMeta(request({ cwd: null, expires_ms: NOW + 10 }), NOW), ['12s', 'expires in 1m']);
});

test('a long folder is shortened so the meta line fits', () => {
  const [folder] = requestMeta(
    request({ cwd: 'C:\\STUFF\\Project-Graphite\\beacon\\spikes\\out\\m3\\run-1', permission_mode: 'bypassPermissions', agent_id: 'a', agent_type: 'general-purpose' }),
    NOW,
  );
  assert.ok(folder.startsWith('…'), folder);
  assert.ok(folder.endsWith('\\run-1'), folder);
  assert.ok(folder.length <= 16, folder);
});

test('MCP tool names split into server and tool', () => {
  assert.deepEqual(mcpTool('mcp__github__get_issue'), { server: 'github', tool: 'get_issue' });
  assert.deepEqual(mcpTool('mcp__claude_ai_Slack__send__now'), { server: 'claude_ai_Slack', tool: 'send__now' });
  assert.equal(mcpTool('Bash'), null);
  assert.equal(mcpTool('mcp__github'), null);
});

test('arguments are compact JSON cut at two hundred characters', () => {
  assert.deepEqual(argumentEntries({ number: 1, labels: ['a', 'b'], title: 'Fix it', nested: { x: null } }), [
    ['number', '1'],
    ['labels', '["a","b"]'],
    ['title', '"Fix it"'],
    ['nested', '{"x":null}'],
  ]);
  const [[, long]] = argumentEntries({ body: 'x'.repeat(500) });
  assert.equal(long.length, 200);
  assert.ok(long.endsWith('…'));
  assert.deepEqual(argumentEntries(null), [['input', 'null']]);
  assert.deepEqual(argumentEntries(['a']), [['input', '["a"]']]);
});

test('URLs split around the host', () => {
  assert.deepEqual(splitUrl('https://docs.example.test/a/b?q=1'), { head: 'https://', host: 'docs.example.test', tail: '/a/b?q=1' });
  assert.deepEqual(splitUrl('http://user@host.test:8080/x'), { head: 'http://user@', host: 'host.test', tail: ':8080/x' });
  assert.deepEqual(splitUrl('not a url'), { head: '', host: '', tail: 'not a url' });
});
