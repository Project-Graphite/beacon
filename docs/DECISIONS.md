# Decisions

Decisions taken while building Beacon, newest phase first. Evidence for the phase 0 entries is in
[SPIKES.md](SPIKES.md). An entry marked **needs Amr** is a proposal until he answers.

## Phase 0 (2026-11-19)

### Where the brief was wrong or incomplete

- **`PermissionRequest` has no `tool_use_id`.** The brief keyed pending decisions on `session_id` +
  `tool_use_id`. A pending decision is keyed on the helper connection instead, and its `tool_use_id`
  is joined from the `PreToolUse` with the same session, agent, tool name and input. The two hooks
  start in either order, so the join tolerates both.
- **Answering elsewhere is not signalled to the hook.** When another approver decides first, the
  pending hook is neither killed nor closed, and a late decision from it is ignored. The card is
  dismissed from later events: a `PostToolUse` with the joined `tool_use_id` means allowed elsewhere;
  a `PostToolBatch` listing that `tool_use_id` with no `PostToolUse` means denied elsewhere. The core
  then releases the helper, which exits 0 and prints nothing.
- **Beacon answering first closes the other approver.** Claude Code sends the host a
  `control_cancel_request` about 13 ms after the hook decides. VS Code and the desktop app are such
  hosts, so their dialog should close by the same path; the real dialogs are still unverified.
- **An interrupted turn fires no `Stop`.** The state machine cannot rely on `Stop` to leave `working`;
  reconciliation (below) covers it.
- **Questions and plans go through `PermissionRequest`, not a blocking `PreToolUse`.** Both work, but
  a blocking `PreToolUse` hides the native question until the hook returns, while `PermissionRequest`
  runs beside the native prompt. The answer is `decision.behavior: "allow"` with `updatedInput`
  (`questions` echoed plus `answers`).
- **`ExitPlanMode` arrived with an empty `tool_input`** in hooks, against the docs. The plan text is
  read from the matching tool-use block in the transcript, falling back to "open in session".
- **Timeouts fail open, and headless runs must not hang.** A hook past its `timeout` is killed and
  counts as no decision. `PermissionRequest` is registered with an explicit `timeout` of 3600 s. For
  an unattended session (`CLAUDE_CODE_SESSION_ATTENDED=0`, as in `claude -p`) the helper returns no
  decision at once, because holding the call would turn an instant denial into an hour-long stall;
  answering headless runs from Beacon is an opt-in setting.

### Fail open on Windows

The helper is registered in exec form as a real `.exe` with no `onFailure`, always exits 0, and
writes to stdout only a complete decision. It gives the core 150 ms to accept the connection. Hooks
that are never registered: `WorktreeCreate` and `WorktreeRemove` (they replace git behaviour),
`PreModelSwitch` (a timeout blocks the switch), `MessageDisplay` and `FileChanged`.

### Hook registration, changed from the brief's table

- `Stop` is registered blocking. The helper returns at once when no prompt is queued, and otherwise
  returns `hookSpecificOutput.additionalContext`, which reaches Claude as "Stop hook additional
  context" with no error notice. `decision: "block"` works too but shows "stop hook reported a
  problem".
- `PostToolBatch` is added: it is the denial signal above.
- `Notification` stays for `idle_prompt`, quota notices and sandbox network prompts, which fire no
  `PermissionRequest`. It is not the permission signal, because it waits about six seconds.

### Discovery, liveness and surface

- Hooks are the live signal. `claude agents --json`, which the docs name as the supported outside
  interface, seeds the registry at start and reconciles it on a slow timer; it lists terminal, VS Code
  and desktop sessions with `pid`, `status` and `waitingFor`. Its `kind` is not a surface: it reported
  a `-p` run as `interactive`.
- The owning process is `CLAUDE_PID` from the helper's environment, watched through a process handle,
  so liveness costs nothing while idle.
- The surface comes from `CLAUDE_CODE_ENTRYPOINT` in the helper's environment (`sdk-cli`,
  `claude-vscode`, `claude-desktop`, `cli`), then the ancestor chain for the terminal or editor. The
  chain is read natively; the PowerShell query the spike used took 628 ms.
- `transcript_path` is taken from hook payloads and never derived from the working directory.
- `~/.claude/sessions/<pid>.json` holds the same facts but is internal; it is not used.

### Sending prompts

| Tier | Path | Phase 0 result | Decision |
|---|---|---|---|
| 1 | Sessions Beacon starts, streaming input | Works; one session id across turns | Build |
| 2 | Queue until `Stop` | Works | Build, with `additionalContext` |
| 3 | Idle wake through an `asyncRewake` Stop hook | Works, but Claude is told the text is an automated notification and "not user input" | Build, labelled as such; never for approvals |
| 4 | Terminal injection | tmux, WezTerm and kitty are not installed | Not in v1 |
| 5 | `claude -p --resume` | Against a live session both write one transcript and the live one never sees the turn | Only for sessions whose process has exited |
| 6 | Clipboard and jump | Not a spike | Build as the fallback |

The session inbox pipe (`CLAUDE_CODE_MESSAGING_SOCKET`) accepted the documented auth line and then
nothing; its message format is undocumented, so it is not used.

### What Claude Code already ships

- **Agent view** lists background sessions only and cannot answer a permission prompt without
  attaching. Beacon reads its data through `claude agents --json` and does not replace it.
- **Channels** have a permission relay, but a session must be started with `--channels` and an
  allowlisted plugin, so it cannot cover sessions that are already running.
- **Remote Control** has no local interface.
- **Mods** could do this in-process, but need a plugin install and are limited by the default guard.
  Command hooks stay the mechanism.

### Usage metrics (added to the brief on 2026-11-19)

- **Per session:** tokens and context fill come from transcripts, one usage per `message.id`, with
  `<session>/subagents/` included. The reader is an isolated adapter with a self-test, because the
  format is internal. Cost is shown as a list-price estimate and labelled so.
- **Overall 5-hour and weekly limit %:** read `cachedUsageUtilization` from `~/.claude.json` and show
  its age. Refresh on demand with one `claude -p` Haiku run that reads `rate_limit_event`, and at most
  every 15 minutes while a session is working. Both are internal or undocumented, so this sits behind
  the same kind of adapter. **Needs Amr:** the refresh cadence, since each refresh spends quota.
- **Overall spend:** not shown as a bill. No local source is authoritative.
- OpenTelemetry to a loopback receiver is the documented alternative for per-session numbers. It is
  not the default: it needs `env` entries in the user settings, sends the account email in every
  export, and carries no limit data.

### Look and stack

- **Graphite look, own components.** Beacon copies the Graphite tokens (`paper`, `surface`, `line`,
  `ink`, `muted`, `live`, `wip`, `danger`, `dormant`, the radii, `--ease-settle`, the system sans and
  mono stacks) into its own stylesheet and imports nothing from `graphite-ui` or MUI. It is dark only,
  as Graphite is, which replaces the brief's follow-the-OS light and dark. `ink` is the accent; `wip`
  marks needs-input, `live` done, `danger` error.
- **React 19, TypeScript, Vite and Tailwind 4** for the UI, as in every other Graphite app. The size
  difference from Svelte is noise next to WebView2.
- **Tauri 2 with a Rust core, and a native Rust helper.** A Node helper measured 46 ms per event
  against the 15 ms budget. Visual Studio Build Tools 2026 with the C++ tools and WebView2 157 are
  installed; Rust is not. **Needs Amr:** approval to install the Rust toolchain with rustup.
- **The long-running process is called the core, not the hub** (`crates/core`), because Graphite Hub
  already exists.
- **Risk: the 80 MB idle budget.** A WebView2 window that is always on screen may not fit. It is
  measured first in M2; the fallback is a natively drawn collapsed pill with the WebView created on
  expand.

### Deferred

- `.graphite.yml` is added in M1, once the stack is approved and the tags are known.
- Antigravity is not installed on this machine, so S8 is open. **Needs Amr.**
- The interactive terminal, VS Code and desktop surfaces are unmeasured until the manual probe in
  SPIKES.md is run. They gate M4 (jump to window, surface icons), not M1.
