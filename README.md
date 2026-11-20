# Beacon

Beacon is a Windows tray app that shows every running Claude Code session and lets you answer
their permission prompts, questions and plan approvals from one panel. It works through Claude
Code's own command hooks: a small helper forwards each hook event to a local core, and the panel
answers the blocking ones. A session behaves exactly as without Beacon when Beacon is not running.

## Requirements

- Windows 11.
- Claude Code 2.1.3xx.
- Rust (1.100 or later) and Node with npm, only for building.

## Build and run

Build the helper and the CLI, then the app, all with the fast profile:

```powershell
cargo build --profile fast -p beacon-hook -p beacon-cli
cd app
npm install
npx tauri build --no-bundle -- --profile fast
```

The binaries land in `target\fast\`: `beacon-app.exe` (tray, pill and panel), `beacon.exe` (the
CLI) and `beacon-hook.exe` (the helper the hooks run). Keep the three in one folder: installing the
hooks copies the helper from next to the running executable.

The app must be built through the Tauri CLI as above. A plain `cargo build` of `beacon-app` loads
the development server instead of the bundled front end.

Start `beacon-app.exe`. It hosts the core itself; if a `beacon daemon` is already running for the
same Beacon home, it connects to that one instead. `--open-panel` opens the panel at start and
`--tray-only` starts without the pill.

## Install the hooks

Open the panel, choose the gear icon in the header and use `Install` under `Claude Code hooks`, or
run `beacon install`. Either way you see the plan with the diff first, and nothing is written until
you choose `Apply` (or run `beacon install --yes`).

Install copies `beacon-hook.exe` into `<Beacon home>\bin`, backs up the Claude Code settings file
into `<Beacon home>\backups`, and adds one Beacon handler to each of the 22 hook events in
`%USERPROFILE%\.claude\settings.json` (or `%CLAUDE_CONFIG_DIR%\settings.json`). Everything else in
the file keeps its order. Installing again repairs a damaged registration.

Uninstall restores the original bytes of the settings file when it has not changed since install.
If it has changed, only Beacon's handlers and the empty containers install created are removed.
Running sessions pick up hook changes without a restart.

## Using it

### The pill

The pill is a small always-on-top strip with one mark per session, at most ten and then `+N`,
followed by the number of sessions that need you. Each state has its own shape as well as colour:

| Mark | Meaning |
|---|---|
| Rotating arc | Working |
| Amber circle with a halo | Needs you: a permission request, a question, a plan or an MCP input form |
| Pause glyph | Stalled: working, but nothing has happened for the configured number of minutes |
| Green check | Done: the turn finished |
| Red triangle | Error: the turn ended with an API error |
| Hollow ring | Idle |

The pill hides while a fullscreen window is in front on its monitor, snaps to edges and corners,
and remembers its position per monitor layout.

### The panel

The panel has three modes:

- Hover: opens when the cursor rests on the pill and closes 400 ms after it leaves both.
- Sticky: opened with a click on the pill, the hotkey, the tray or a second launch; it stays until
  you close it with Escape or the same action.
- Pinned: while any session needs you, the panel stays open and cannot be dismissed. When nothing
  needs you any more it returns to the mode it was opened in.

Sessions are grouped by state. Arrow keys select a session, Enter expands it, `a` acknowledges, `s`
snoozes it for 5 minutes, `j` jumps to its window and `/` opens the prompt box.

### Answering requests

Each pending request is a card with the tool, its input and the session's folder. Nothing is sent
without a click or a key press on that card; there is no "allow all".

- `Allow` (`A`), `Deny` (`D`) and `Deny with message` (`M`) answer the current request. Tab and
  Shift+Tab move between requests when several are waiting. `More` offers `Deny and stop Claude`.
- A request that looks destructive is flagged. Its `Allow` needs a second deliberate press within
  4 seconds, and `Always allow` is not offered for it.
- `Always allow…` opens the rule, prefilled from Claude Code's suggestion, with a scope: `This
  session`, `This project, only me` (`.claude/settings.local.json`), `This project, shared`
  (`.claude/settings.json`) or `All my projects` (user settings). Claude Code saves it as an
  ordinary permission rule; Beacon keeps no rules of its own.
- Secrets in a request are masked until you ask to see them.
- A question shows its options, with `Other` for free text. A plan shows its text with `Approve
  plan`, `Keep planning` and `Reject with feedback`.
- If the request is answered in the session first, the card closes and says so.

### Jump

`Jump` raises the window that runs the session: the terminal, the editor or the desktop app,
found through the process tree. For a VS Code session it then opens the session's tab through the
`vscode://anthropic.claude-code` link.

### Sending a prompt

The prompt box in an expanded session sends text by one of three paths:

| Session | Path |
|---|---|
| Turn running | Queued and handed to Claude when the turn ends, through the `Stop` hook |
| Idle in VS Code | The session opens in VS Code with the text in its prompt box, unsent |
| Idle elsewhere | The text goes to the clipboard and the window is raised |

A queued prompt shows `queued`, `delivered`, `acknowledged` or `failed`, and survives a restart.

### Snooze and acknowledge

Snooze silences one session, or all of them from the bell in the header or the tray, for 5 or 15
minutes: no pinned panel, no count on the pill and no notification. Acknowledge clears a `done`,
`error` or `stalled` session. Bringing a finished session's window to the front acknowledges it
when that window holds exactly one session.

### Notifications

Windows notifications announce sessions that start needing you or finish. Sessions that change
within 2 seconds of each other share one notification, and the session whose window is in front
gets none. Both kinds can be turned off in the settings view.

### Settings

The gear icon in the panel header opens the settings view; Escape returns to the sessions. It holds
the hooks status with install, repair and uninstall; whether headless runs are answered from
Beacon; the stall time; notifications; the panel shortcut; start at sign-in; and the version, the
Beacon home and its logs folder.

### Tray menu

`Open panel`, `Snooze all for 5 minutes`, `Snooze all for 15 minutes`, `Clear snooze`, `Hide pill`
or `Show pill`, `Show on` (the display the pill and panel use), `Never show on` (displays to
avoid), `Position` (move the pill to a corner) and `Quit`.

### CLI

| Command | What it does |
|---|---|
| `beacon daemon` | Runs the core without the app |
| `beacon stop` | Stops the core |
| `beacon status [--watch] [--json]` | Lists sessions and pending requests |
| `beacon decide <id> allow` | Allows a pending request |
| `beacon decide <id> deny [--message <text>] [--stop]` | Denies it, optionally with a message or stopping Claude |
| `beacon decide <id> answer "<question>=<label>" ...` | Answers a question |
| `beacon decide <id> always "<Tool(specifier)>" [--scope session\|local\|project\|user]` | Allows it and saves a rule |
| `beacon ack <session>` | Acknowledges a session |
| `beacon jump <session>` | Raises the session's window |
| `beacon where <session>` | Prints the window the session runs in |
| `beacon prompt <session> "<text>"` | Queues a prompt for the end of the current turn |
| `beacon snooze <session\|all> <minutes>` | Snoozes one session or all |
| `beacon unsnooze <session\|all>` | Clears a snooze |
| `beacon usage [--refresh]` | Shows token use per session and the account limits |
| `beacon install [--settings <path>] [--yes]` | Shows or applies the hook installation |
| `beacon uninstall [--settings <path>] [--yes]` | Shows or applies the removal |
| `beacon doctor [--settings <path>]` | Checks the installation, the core and a helper round trip |

`<session>` is a session id or a unique prefix of one.

## What works where

| | Terminal | VS Code extension | Desktop app Code tab | Headless (`claude -p`) |
|---|---|---|---|---|
| Session state | Built, not verified on this surface | Built, not verified on this surface | Built, not verified on this surface | Verified |
| Answer permission requests | Built, not verified on this surface | Built, not verified on this surface | Built, not verified on this surface | Verified [1] |
| Answer questions and plans | Built, not verified on this surface | Built, not verified on this surface | Built, not verified on this surface | Verified [1] |
| Jump to window | Built, not verified on this surface | Built, not verified on this surface [2] | Built, not verified on this surface [2] | Not supported [3] |
| Send a prompt | Built, not verified on this surface | Built, not verified on this surface | Built, not verified on this surface | Verified [4] |
| Usage | Built, not verified on this surface | Built, not verified on this surface | Built, not verified on this surface | Verified |

1. Only with `Answer headless runs from Beacon` on. Off (the default), a run with nobody attached
   is passed straight through so it never waits.
2. Finding the window was checked read-only for running VS Code and desktop sessions. The focus
   change and the VS Code link are not verified on any surface.
3. A headless run has no window.
4. The prompt at turn end. A headless run has no window for the other two paths.

Every end-to-end check so far ran against headless `claude -p` sessions: state, allow, deny,
always allow, a question, a plan, a prompt at turn end and usage. The hooks have not been installed
for the interactive terminal, the VS Code extension or the desktop Code tab, so those surfaces are
built but unverified.

## Limits and known gaps

- The helper adds about 30 ms to each hook event, which is process creation on Windows. A session
  waits for it only on the five blocking events: `PermissionRequest`, `Elicitation`, `Stop`,
  `SessionEnd` and `StopFailure`.
- MCP input forms are not answered from Beacon. The card says what is asked and points to the
  session, because holding the hook would hide Claude Code's own form.
- Notifications have no action buttons.
- A session that the default-terminal setting handed to Windows Terminal has no Windows Terminal
  ancestor, so jump does not find its window.
- Per-session usage is read from the transcript, whose format is internal to Claude Code and may
  change. The account limits come from a cached value in `~/.claude.json`, also internal.
- Several sessions in one editor window cannot be told apart, so "no notification for the focused
  session" and "focus acknowledges a finished session" apply only when one session maps to the
  window in front.

## Security and privacy

- The helper talks to the core over a named pipe with a random name that rejects remote clients and
  grants access to the current user only. Each connection must present a 256-bit token from the
  `endpoint` file in the Beacon home.
- Beacon opens no network connection. `Refresh` on the usage figures runs one `claude -p` Haiku
  request through Claude Code, and only when you ask for it.
- It fails open. The helper exits 0 on every path, prints only a complete decision, and gives up
  after short timeouts, so a stopped or hung core never blocks a session. Permission requests wait
  for you, up to the one-hour timeout the hook is registered with.
- The one-line request preview and the activity line are redacted in the core before any front end
  sees them. Everything from a session is shown as plain text.
- The helper forwards the hook payload and an allowlist of 18 environment variables; no other
  variable leaves the session.

What is stored where:

| Place | Contents |
|---|---|
| `%LOCALAPPDATA%\Beacon` (or `BEACON_HOME`) | `endpoint` (pipe name and token, removed on a clean stop), `state.json` (sessions, snoozes, queued prompts), `config.json`, `ui.json` (pill positions, displays, shortcut, notification choices), `install.json`, `bin\beacon-hook.exe`, `backups\` (copies of the settings file), `logs\core.log`, `icon.png` |
| Claude Code `settings.json` | The Beacon hook handlers |
| `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value `Beacon` | Only while `Start Beacon when I sign in` is on |
| `HKCU\Software\Classes\AppUserModelId\com.project-graphite.beacon` | The name and icon Windows shows on Beacon's notifications |

## Uninstall

1. In the settings view choose `Uninstall` and then `Apply`, or run `beacon uninstall --yes`.
2. Turn off `Start Beacon when I sign in`, then choose `Quit` in the tray menu.
3. Delete the Beacon home folder and the binaries.
4. Optionally remove the notification identity:
   `reg delete HKCU\Software\Classes\AppUserModelId\com.project-graphite.beacon /f`.

## More

[docs/DECISIONS.md](docs/DECISIONS.md) records the design decisions and what was verified, and
[docs/SPIKES.md](docs/SPIKES.md) the experiments behind them.
