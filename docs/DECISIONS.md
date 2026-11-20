# Decisions

Decisions taken while building Beacon, newest phase first. Evidence for the phase 0 entries is in
[SPIKES.md](SPIKES.md). An entry marked **needs Amr** is a proposal until he answers.

## M3 to M5 and display targeting (2026-11-20)

### Scope cut for v1

Asked to drop what is not needed: the VS Code companion extension, Antigravity support, the claude.ai
browser extension, the desktop-chat adapter, Beacon-spawned sessions, idle wake, headless resume, the
edge-reveal form, toast action buttons and the test-matrix document are all out of v1.

### Answering from the panel (M3)

- Nothing is sent without a click or key press on that request. There is no "allow all".
- A request that looks destructive is flagged and its Allow needs a second deliberate press. The
  detector only adds that step; it never denies.
- "Always allow" sends one `addRules` entry through `updatedPermissions`, with the scope the user
  picks. Claude Code saves it as an ordinary permission rule; Beacon keeps no rules.
- Secrets are masked in the request view until the user asks to see them. The rule editor is
  prefilled unmasked, so a saved rule is never a masked one.
- Questions are answered with `updatedInput` (`questions` plus `answers`); a plan is approved by
  echoing its `tool_input`. Claude Code 2.1.328 now passes the plan text to hooks, so reading it from
  the transcript is only a fallback.
- **MCP input forms are not answered by Beacon.** A blocking `Elicitation` hook hides Claude Code's
  own dialog until it returns, which would break the rule that a session behaves as if Beacon were
  absent. The card says what is asked and points to the session.
- Verified against real Claude Code: allow, deny with message, always allow, the destructive two
  steps, a question and a plan approval.

### Jump to window (M4)

- No editor extension. The window is found from the process tree: the ancestor chain of the
  session's `claude.exe` leads to the editor, the desktop app, Windows Terminal or a console, and the
  window whose title carries the session's folder name is chosen.
- For a VS Code session, the documented link `vscode://anthropic.claude-code/open?session=<id>` then
  focuses that session's tab. It is opened only after the window was focused, because the link acts
  on whichever VS Code window is in front.
- The focus change runs in the process the user just clicked in, since Windows only lets that one
  change the foreground window. From the CLI it is usually refused and the taskbar button flashes.
- Several sessions in one editor window cannot be told apart, so the rules "no toast for the focused
  session" and "focus acknowledges a finished session" apply only when exactly one session maps to
  the foreground window.
- Not verified: the focus change from a panel click, and the link. A session handed to Windows
  Terminal by the default-terminal setting has no Windows Terminal ancestor and is not found.

### Sending a prompt (M5, trimmed)

| Session | Path | What happens |
|---|---|---|
| Turn running | At turn end | The core queues the text and returns it from the `Stop` hook as `additionalContext`. |
| Idle in VS Code | Prefill | The session opens in VS Code with the text in its prompt box, unsent. |
| Idle elsewhere | Copy and jump | The text goes to the clipboard and the window is raised. |
| Ended | None | |

- A queued prompt shows `queued`, `delivered`, `acknowledged` or `failed` with a reason, and
  survives a core restart. A turn that ends without a `Stop` fails its queued prompts instead of
  leaving them queued.
- A `Stop` that delivers a prompt is not the end of the turn.
- Verified against real Claude Code: a prompt queued during a running command was delivered at
  `Stop` and Claude answered it.

### Display targeting

- `ui.json` holds the chosen display and a list of displays to avoid. The pill and the panel appear
  only on the target: the chosen display if connected and allowed, otherwise the primary if allowed,
  otherwise the first allowed display, otherwise nowhere (tray only).
- The tray menu has `Show on`, `Never show on` and `Position`. Dragging the pill onto an allowed
  display selects it; a drop on an avoided display is undone.
- The single-instance lock is skipped when `BEACON_HOME` is set, so a test instance can run beside
  the real one. The core still allows one instance per home.

### Settings and Windows integration

- The panel has a settings view: hook install, repair and uninstall (the plan and its diff are shown
  before anything is written), headless answering, the stall time, notifications, the shortcut and
  start at sign-in.
- There is no installer. At start the app registers its notification identity under
  `HKCU\Software\Classes\AppUserModelId`, so Windows attributes toasts to Beacon, and start at
  sign-in is a `Run` value under `HKCU`. A build run from `target\` skips the registration.

### Build

`[profile.fast]` (no link-time optimisation) is for day-to-day and verification builds. The app must
be built through the Tauri CLI, or it loads the dev server instead of the bundled front end.

## M2: tray, pill and panel (2026-11-20)

### The pill is native; the panel is a WebView that exists only while it is open

Measured on this machine with a release build, 60 s of sampling after a 60 s warm-up:

| State | Processes | Private memory | Working set | CPU over 60 s |
|---|---|---|---|---|
| WebView2 pill, visible | 7 to 8 | 164 MB | 380 MB | 0.05 s |
| WebView2 pill, hidden | 7 to 8 | 159 MB | 372 MB | 0.02 s |
| Native pill and tray, no sessions | 1 | 4.2 MB | 18 MB | 0.00 s |
| Native pill animating, panel closed | 1 | 8.7 MB | 39 MB | 0.33 s |
| Panel open | 7 | 170 MB | 386 MB | 0.23 s |

A live WebView2 window costs about 160 MB whether it is shown or hidden, against an idle budget of
80 MB, so the collapsed pill is a Win32 layered window painted by Beacon's own rasterizer. The panel
window is created when it opens and destroyed when it closes, never hidden. Opening it from cold
takes about 0.3 s. While a session needs the user the panel is pinned open, and the app is over the
idle budget for as long as that lasts.

### Pill

- One mark per session, at most ten and then `+N`, each state with its own shape as well as colour:
  working is a rotating arc, the four needs-input states a solid amber circle with a halo, stalled a
  pause glyph, done a green check, error a red triangle, idle a hollow ring. A count of the sessions
  that need the user follows the marks.
- The animation timer runs only while something on the pill animates and reduced motion is off.
- Fully transparent pixels pass clicks through. The pill never takes focus.
- It hides while a fullscreen window is in the foreground on its monitor. That is driven by the
  foreground-change hook, so a window that goes fullscreen while already in the foreground is not
  noticed until the foreground changes.
- Its position is stored per monitor layout in `ui.json` and snaps to edges and corners.

### Panel

- Three modes. Hover closes 400 ms after the cursor leaves the pill and the panel. Sticky (a click,
  the hotkey, the tray, a second launch) stays until dismissed. Pinned applies while any session is
  alerting and cannot be dismissed; when alerting ends the panel returns to the mode it was opened
  in.
- Hover and alert opens do not take focus.
- The page can call a fixed list of commands and nothing else, restricted to the panel window by a
  capability file. The CSP is `default-src 'self'` plus the `connect-src` Tauri's own IPC needs.
- Everything from a session is rendered as text nodes. The last message goes through a hand-written
  renderer for a Markdown subset; there is no HTML injection path and no Markdown dependency.
- The panel is a solid Graphite surface. The brief's OS blur is not used, because Graphite is flat.

### Attention, acknowledge and snooze

- `alerting` means in an attention state and not snoozed. The pill count, the pinned panel, the tray
  icon and toasts all follow it.
- Acknowledge clears `done`, `error` and `stalled`. The brief had it only for `done`, which would
  leave an API error pinning the panel until the next prompt.
- Snooze is per session or global, for 5 or 15 minutes, held in the core so the CLI and the app
  agree, and it survives a restart.

### Toasts

- The first snapshot after start is a baseline. Sessions that start alerting or finish within 2 s of
  each other become one toast. Snoozed sessions produce none.
- Toasts have no action buttons; those need the packaged app identity.

### Hotkey

`Ctrl+Alt+Space` is already registered by another program on this machine, so it never fired. The
accelerator is read from `hotkey` in `ui.json` and can be changed in the settings view.

### Usage, as built

- Per session: tokens and context size from the transcript, one usage per `message.id`, subagent
  files included, read only at turn boundaries. There is no price table; a cost is shown only when
  the transcript itself reports one, which Claude Code writes when a session closes.
- Account: the 5-hour and weekly windows come from the cached value in `~/.claude.json`. A refresh
  runs one Haiku request and only when asked for. There is no automatic probing.

### Other

- The app hosts the core but reads it through the same control subscription the CLI uses, so there
  is one data path. If a `beacon daemon` is already running, the app connects to it instead.
- A session known only from `claude agents --json` gets its surface from its process image path:
  VS Code and its forks, or the desktop app. A terminal and a headless run share one binary and stay
  unknown until a hook fires.
- The one-line preview of a pending request and the activity line are redacted in the core, so no
  front end sees the raw text.
- A light theme is not planned, because Graphite is dark only.

## M1: helper, core, installer and CLI (2026-11-19)

### Shape

- Four crates: `protocol` (wire format, no external crates), `hook` (the helper `beacon-hook.exe`, no
  external crates), `core` (registry, state machine, decision broker, pipe server, control channel)
  and `cli` (the `beacon` binary). External crates are `serde`, `serde_json`, `tokio` and
  `windows-sys` only.
- Until the M2 app hosts the core, `beacon daemon` does. One core per Beacon home, enforced with a
  named mutex.
- The core writes `endpoint` (pipe name and a 256-bit token) into the Beacon home at start and removes
  it on a clean stop. The pipe has a random name, rejects remote clients and grants access to the
  current user only. A connection with a bad token is closed.
- The same pipe carries a control channel (newline-delimited JSON: `snapshot`, `subscribe`, `decide`,
  `acknowledge`, `shutdown`) that the CLI uses now and the UI will use later.

### Helper

- It exits 0 on every path, including a panic, and writes to stdout only the bytes of a decision the
  core sent. It forwards the raw payload untouched and an allowlist of 18 environment variables; no
  other variable leaves the session.
- A watchdog ends it if the request is not sent within 2 s, or if a turn-end reply does not arrive
  within 1 s, so a hung core cannot hold a session. Permission requests and MCP input wait without a
  deadline, because the wait is for the person.
- **The 15 ms budget is not met: an event costs about 30 ms.** That is process creation on this
  machine; an empty run of the helper takes 31 ms and `whoami.exe` 34 ms. It delays a session only on
  the five blocking events below. HTTP hooks would avoid the process, but they show a hook error in
  every session while the core is down and cannot report the session's environment, so the helper
  stays.

### Hook registration

Twenty-two events, kept in one constant that install, uninstall, doctor and the tests share.

- Blocking: `PermissionRequest` and `Elicitation` (`timeout` 3600), `Stop` (`timeout` 10),
  `SessionEnd` and `StopFailure` (blocking because Claude Code kills async hooks at teardown, and
  these are the last events of a run).
- `async: true`: `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
  `PostToolUseFailure`, `PostToolBatch`, `PermissionDenied`, `Notification`, `SubagentStart`,
  `SubagentStop`, `TaskCreated`, `TaskCompleted`, `PreCompact`, `PostCompact`, `CwdChanged`,
  `PostModelSwitch`, `ElicitationResult`.
- Async hooks race each other, so the core accepts a `PreToolUse` on either side of its
  `PermissionRequest`.

### Installer

- `install` and `uninstall` print a plan with a diff and change nothing until they are run with
  `--yes`. Both back up the settings file first.
- Install removes any Beacon handler it finds, then adds the current set, so it is also the repair.
  Everything else keeps its order.
- Uninstall restores the original bytes when the file is unchanged since install, which is what makes
  install then uninstall byte-identical. If the file changed in between, it removes Beacon's handlers
  structurally and drops only the empty containers install created.
- A structural write re-serializes the file. Indentation, line ending, BOM and trailing newline are
  kept, but spellings `serde_json` writes differently (`é`, `1e3`, inline arrays) are normalized.
  Claude Code writes the file in the same style, so real files are unaffected.
- A file that is not a JSON object is refused and left untouched.
- A helper that is still running cannot be deleted on Windows, so it is renamed aside and removed
  later.
- Doctor does not read the registry-delivered managed policy and says so.

### Behaviour worth knowing

- A malformed `state.json` or `config.json` stops the core with an error instead of being discarded.
- Ended sessions are dropped from the registry 24 hours after their last event.
- A session known only from `claude agents --json` is marked limited until one of its hooks fires.
- The end-to-end check ran against a real `claude -p` session with a temporary settings file: allow
  from `beacon decide` ran the tool, deny returned the message to Claude, and uninstall restored the
  file byte for byte.

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
  format is internal. M2 dropped the cost estimate: there is no reliable price table.
- **Overall 5-hour and weekly limit %:** read `cachedUsageUtilization` from `~/.claude.json` and show
  its age. Refresh on demand with one `claude -p` Haiku run that reads `rate_limit_event`. Both are
  internal or undocumented, so this sits behind the same kind of adapter. M2 built it without any
  automatic refresh, since each one spends quota.
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
