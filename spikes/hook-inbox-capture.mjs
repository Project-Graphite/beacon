import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { readStdin, resolveOutDir } from './spike-log.mjs';

try {
  await readStdin();
  const outDir = resolveOutDir(process.argv[2]);
  mkdirSync(outDir, { recursive: true });
  writeFileSync(
    join(outDir, 'inbox.json'),
    JSON.stringify({
      socket: process.env.CLAUDE_CODE_MESSAGING_SOCKET,
      token: process.env.CLAUDE_CODE_MESSAGING_TOKEN,
      pid: process.pid,
      ppid: process.ppid,
    }),
  );
} catch {}
process.exitCode = 0;
