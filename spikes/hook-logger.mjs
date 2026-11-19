import { execFileSync } from 'node:child_process';
import { appendEvent, monoNow, readStdin, resolveOutDir } from './spike-log.mjs';

const watchedEnvName = /^(CLAUDE|TERM|WT_|VSCODE|TMUX|KITTY|WEZTERM|ANTIGRAVITY|CURSOR|ELECTRON|ConEmu|SESSIONNAME|WSL|MSYSTEM|COLORTERM)/i;
const sensitiveEnvName = /TOKEN|KEY|SECRET|PASSWORD|AUTH|CREDENTIAL|EMAIL|UUID/i;

function watchedEnv() {
  return Object.fromEntries(
    Object.entries(process.env)
      .filter(([name]) => watchedEnvName.test(name))
      .map(([name, value]) => [name, sensitiveEnvName.test(name) ? `<redacted:${value.length}>` : value]),
  );
}

function ancestorChain() {
  const processes = JSON.parse(
    execFileSync(
      'powershell.exe',
      ['-NoProfile', '-Command', 'Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name | ConvertTo-Json -Compress'],
      { encoding: 'utf8', windowsHide: true },
    ),
  );
  const byPid = new Map(processes.map((entry) => [entry.ProcessId, entry]));
  const chain = [];
  const seen = new Set();
  for (let entry = byPid.get(process.pid); entry && !seen.has(entry.ProcessId); entry = byPid.get(entry.ParentProcessId)) {
    seen.add(entry.ProcessId);
    chain.push({ pid: entry.ProcessId, name: entry.Name });
  }
  return chain;
}

try {
  const [label, explicitOutDir] = process.argv.slice(2);
  const raw = await readStdin();
  const startedAt = { ts: new Date().toISOString(), mono: monoNow() };
  const payload = JSON.parse(raw);
  const event = payload.hook_event_name;
  const record = { ...startedAt, label, event, payload, env: watchedEnv() };
  if (event === 'SessionStart' || event === 'PermissionRequest') {
    try {
      record.chain = ancestorChain();
    } catch (error) {
      record.chainError = String(error);
    }
  }
  appendEvent(resolveOutDir(explicitOutDir), record);
} catch {}
process.exitCode = 0;
