import assert from 'node:assert/strict';
import { test } from 'node:test';
import { redact, redactValue } from './redact.ts';

test('secret assignments keep the name and mask the value', () => {
  assert.equal(redact('GITHUB_TOKEN=abc123 npm publish'), 'GITHUB_TOKEN=*** npm publish');
  assert.equal(redact('$env:OPENAI_API_KEY="s3cr3t value"; run'), '$env:OPENAI_API_KEY=***; run');
  assert.equal(redact("mysql --password='p w' db"), 'mysql --password=*** db');
  assert.equal(redact('DB_PASSWD=hunter2'), 'DB_PASSWD=***');
  assert.equal(redact('client_secret=xyz&a=1'), 'client_secret=***');
  assert.equal(redact('set private_key=-----BEGIN'), 'set private_key=***');
});

test('ordinary assignments are left alone', () => {
  for (const text of ['NODE_ENV=production npm start', 'count=3', 'KEY=', 'echo TOKEN is unset', 'cd C:\\keys && dir']) {
    assert.equal(redact(text), text);
  }
});

test('authorization headers and bearer tokens are masked', () => {
  assert.equal(
    redact('curl -H "Authorization: Basic dXNlcjpwYXNz" https://x.test'),
    'curl -H "Authorization: ***" https://x.test',
  );
  assert.equal(redact('authorization: token abc\nnext line'), 'authorization: ***\nnext line');
  assert.equal(redact("curl -H 'Bearer eyJhbGciOi.payload.sig' x"), "curl -H 'Bearer ***' x");
  assert.equal(redact('Bearer'), 'Bearer');
});

test('long prefixed tokens are masked', () => {
  assert.equal(redact('use sk-ant-api03-abcdefghijklmnop now'), 'use *** now');
  assert.equal(
    redact('git push https://ghp_0123456789abcdefABCDEF@github.com/x'),
    'git push https://***@github.com/x',
  );
  assert.equal(redact('xoxb-123456789012-abcdefghij'), '***');
  assert.equal(
    redact('sk-short and task-sk-0123456789abcdefghij'),
    'sk-short and task-sk-0123456789abcdefghij',
  );
});

test('spaced, quoted and flag forms are masked', () => {
  assert.equal(redact('$env:GITHUB_TOKEN = "abc"; git push'), '$env:GITHUB_TOKEN = ***; git push');
  assert.equal(redact('{"api_key":"abc123","n":1}'), '{"api_key":***,"n":1}');
  assert.equal(redact('password: "hunter2"'), 'password: ***');
  assert.equal(redact('mysql --password hunter2 db'), 'mysql --password *** db');
  for (const text of ['apiKey: string', 'git commit -m "fix: token refresh"', '--api-key']) {
    assert.equal(redact(text), text);
  }
});

test('url credentials lose their password', () => {
  assert.equal(redact('psql postgres://user:pw@host/db'), 'psql postgres://user:***@host/db');
  assert.equal(redact('curl https://host:8080/x'), 'curl https://host:8080/x');
});

test('more token shapes are masked', () => {
  assert.equal(redact('github_pat_11ABCDEFG0123456789_abcdefghij x'), '*** x');
  assert.equal(redact('AKIAIOSFODNN7EXAMPLE'), '***');
  assert.equal(redact('glpat-abcdefghij0123456789'), '***');
  assert.equal(redact('token eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.sig_value'), 'token ***');
  assert.equal(
    redact('a\n-----BEGIN PRIVATE KEY-----\nMIIabc\n-----END PRIVATE KEY-----\nb'),
    'a\n-----BEGIN PRIVATE KEY-----***\nb',
  );
});

test('a value under a secret name is masked whole', () => {
  assert.equal(redactValue('api_key', '"abc"'), '***');
  assert.equal(redactValue('title', '"Fix it"'), '"Fix it"');
  assert.equal(redactValue('body', '"use sk-ant-api03-abcdefghijklmnop"'), '"use ***"');
});

test('non-ASCII text survives', () => {
  assert.equal(redact('é API_KEY=ж日本 ü'), 'é API_KEY=*** ü');
});

test('the preview vectors of the core match', () => {
  assert.equal(redact('curl -H "Authorization: Bearer abc"\nhttps://x.test'), 'curl -H "Authorization: ***"\nhttps://x.test');
  assert.equal(redact('API_KEY=abc123 npm run deploy'), 'API_KEY=*** npm run deploy');
  assert.equal(redact('https://example.test/?api_key=k'), 'https://example.test/?api_key=***');
});
