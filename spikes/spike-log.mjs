import { appendFileSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const spikesDir = dirname(fileURLToPath(import.meta.url));

export function resolveOutDir(explicitOutDir) {
  return resolve(explicitOutDir || process.env.BEACON_SPIKE_OUT || join(spikesDir, 'out'));
}

export function monoNow() {
  return performance.timeOrigin + performance.now();
}

export async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  return Buffer.concat(chunks).toString('utf8');
}

export function appendEvent(outDir, record) {
  mkdirSync(outDir, { recursive: true });
  appendFileSync(
    join(outDir, 'events.jsonl'),
    JSON.stringify({ ts: new Date().toISOString(), mono: monoNow(), pid: process.pid, ppid: process.ppid, ...record }) + '\n',
  );
}
