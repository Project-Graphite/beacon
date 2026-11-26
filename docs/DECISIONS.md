# Decisions

This is Beacon's design log: the decisions taken while building it, with what was measured or
verified, in dated entries with the newest first. A later entry supersedes an earlier one, and a
statement that no longer holds is marked "(Superseded …)" where it stands. An item marked **open**
is not decided yet. Evidence for the phase 0 entries is in [SPIKES.md](SPIKES.md).

## Audit after 0.2.0 (2026-11-26)

A code audit of 0.2.0 found the gaps below. Most fixes landed with a test; the items left open
follow at the end.

- **More text is redacted at the source.** The core now redacts the last message, an MCP form's
  message and an API error before it stores them, as it already did for previews, titles and
  activity lines. `beacon status --json` redacts request inputs, permission suggestions, plans and
  queued prompts, and the install and uninstall plans redact the settings lines they show.
- **Prompts are refused from inside a Claude session, like decisions.** The core refuses a `prompt`
  request whose pipe client descends from a live session's `claude.exe`. This is still a speed bump,
  not a security boundary (see 2026-11-24).
- **An MCP form gets the 1 s reply budget.** The helper waited for the core's reply to an
  `Elicitation` without a deadline, so a core that accepted the event and then stalled hid Claude
  Code's own dialog for up to the hook's hour. It now gives up after 1 s, as for `Stop`.
- **A session that reconcile finds gone has its pending requests expired**, instead of leaving them
  waiting in the panel.
- **A late prompt event expires only the requests asked before it was sent.** A prompt, stop or
  session-end event used to expire every pending request of its session, so one that arrived late
  could expire the new turn's request. The core now compares each request with the time the helper
  stamped on the event. Limit: the helper stamps it after connecting to the core, so a request asked
  while that helper was connecting still counts as older.
- **A `gitdir:` pointer to a network share is not followed**, so reading a session's branch never
  opens a `\\server` or `//server` path.
- **Titles from hooks and names from `claude agents` are cut to one redacted line of 80 characters**,
  like the other titles.
- **Install and uninstall are safer with the settings file.** Install writes a `before-install` copy
  before it repairs a file that has no restore point, backups are kept ten per kind, a relative
  `--settings` path is stored and compared as an absolute one, and the staged file is flushed to
  disk before it replaces the original. An always rule that starts with a dash is refused instead of
  being read as a flag.
- **The uninstaller keeps the data when it cannot remove the hooks.** It warns that the hooks are
  still in the settings file and keeps `%LOCALAPPDATA%\Beacon`, so the helper they point to and the
  backups remain. It now also removes the notification registration and, when the data is deleted,
  the WebView2 profile in `%LOCALAPPDATA%\com.project-graphite.beacon`.
- **The version lives in the workspace `Cargo.toml` and `app/package.json`**, not in
  `tauri.conf.json`, which now gives the installer a homepage, copyright and description instead. The
  release script refuses a working tree with uncommitted or untracked changes, builds with
  `--locked`, and runs `npm ci` only when `node_modules` is missing.
- **`undismiss` was missing from the panel's allowed commands in 0.2.0**, so Hide's Undo and Show
  failed. It is allowed now, and a test checks that the command handler list, the build manifest and
  the panel's capability name the same commands.
- **The pinned panel holds only while a session is starting, working, stalled or needs you.** In
  0.2.0 an unseen finished or failed session also held it, so it did not close after Claude was done.
  It also closes when a fullscreen window takes the foreground and opens again after.
- **Only the installed build applies the start mode**, so a development build run from `target\`
  no longer takes over starting with Claude or the sign-in entry.
- **Every `ui.json` field is read leniently**: one mistyped value falls back to its default alone
  instead of resetting the file.
- **The overlay palette matches the panel's dark tokens.** The light theme's accents stay at the
  stronger mark grade, so they read on a white capsule.
- **The keys act on one current request.** The marked request is the one `A`, `D` and `M` act on,
  decided in one pure function (`decideKey` in `app/src/lib/keys.ts`) with its own tests. A key acts
  only on a request that is fully in view (the first press scrolls it there) and only once the
  marked request has been in place for 0.4 s, so an answer cannot land on a request that just took
  another's place; request buttons ignore clicks for the same 0.4 s. `Tab` is left to the browser
  unless two or more requests wait and focus is on the page or inside a request.
- **Masking covers plans and questions too.** Plan text, question text and options are masked like
  tool input, each card with its own "Show unmasked". Masking walks the value tree, so a secret
  inside a string argument, or a value under a secret-looking key, is masked in TypeScript and in
  Rust (`redact_json`, which `beacon status --json` also uses).
- **Destructive detection covers more.** Scripts in MCP tool arguments (`command`, `cmd`,
  `script`), an abbreviated `-Recurse`, `Format-Volume`, `Clear-Disk` and `rimraf` are flagged.
- **Screen readers hear new requests, notices and failures**, through a polite live region and
  alerts on decide and prompt failures.

### Open

- (a) The overlay's invisible hit areas catch clicks (pinned by tests). Removing them needs polling
  or a mouse hook, against the goal that an idle Beacon costs nothing.
- (b) "Uninstall" from the setup program's maintenance page keeps the hooks. Tauri passes the
  uninstaller no marker that tells that case from an upgrade.
- (c) `bundle.publisher` is not set, because Tauri derives the registry key that holds the install
  folder from it, and changing it would break upgrades from 0.2.0.
- (d) There is one install record per machine, so installing into a second Claude config folder
  replaces the record of the first.
- (e) With no install record and stale Beacon handlers in the settings file, uninstall restores the
  stale handlers (pinned by a test).
- (f) A request resolved only by `PostToolBatch` is recorded as "Denied in session" even when it was
  allowed natively and its `PostToolUse` arrives second (pinned by a test).
- (g) A `settings.json` that is a symbolic link is replaced by a regular file on write.
- (h) Blocking filesystem work (the git branch lookup, the state write) runs on the core's single
  thread.
- (i) A wake prompt is marked delivered when it is queued, so a helper that dies in that instant
  loses it without a trace.
- (j) With two or more requests waiting, `Tab` steps through the requests, so the controls inside a
  card are reached by pointer or by the letter keys.

## 0.2.0: refined Graphite, light and dark, an installer (2026-11-24)

- **The look is refined Graphite, not a new world.** The owner chose to keep the Graphite dark
  identity and make it calmer and roomier rather than adopt Windows Fluent or a new palette. The panel is 376 px
  wide with 52 px rows, 16 px gutters and Segoe UI Variable; [DESIGN.md](../DESIGN.md) holds the
  tokens.
- **Beacon follows the Windows theme.** The panel follows `prefers-color-scheme`, the pill, tab and
  peek follow `AppsUseLightTheme`, and the tray icon follows `SystemUsesLightTheme`; all redraw on
  `ImmersiveColorSet`.
- **Icons carry no colour.** The app icon is a white pulse (a core and two fading rings) on graphite;
  an earlier amber dot-and-arcs mark read as Wi-Fi. The tray mark uses the taskbar's ink and shows a
  small badge instead of a colour when a session needs you.
- **Two invisible options for the pill.** `Pill background: None` drops the capsule and keeps the
  marks; `Show it: Only when Claude needs you` with `Reveal on hover` keeps the overlay invisible
  until needed, revealed from a thin strip so it never swallows clicks on what lies beneath. Shadows
  sit in click-through windows for the same reason. (Superseded: the reveal strip, like the tab's hit
  band and a background-less pill, does catch clicks on its own small area. That is the accepted
  cost of hover detection without polling; see the open items of 2026-11-26.)
- **The tray menu and the settings view use the same names and choices.** The menu covers snooze,
  the overlay, attention and opens the settings view; the old flat show/hide items are gone.
- **The shortcut is recorded, not typed.** The recorder pauses the global hotkey while it listens and
  asks for two modifiers (or one with a function key) so a shortcut cannot steal Ctrl+C or F5.
- **Releases ship a per-user installer.** Tauri's NSIS bundler installs into
  `%LOCALAPPDATA%\Beacon\app` without admin rights, stops a running Beacon first, offers to connect
  the hooks, starts Beacon, and on uninstall removes the hooks, the Run value and optionally the data.
  A zip of the three executables is kept for people who prefer it. Neither is code-signed yet.

## Self-approval check and the wake hook's lifetime (2026-11-24)

- **Refusing decisions from inside a Claude session is a speed bump, not a security boundary.** The
  core refuses a `decide` request whose pipe client descends from a live session's `claude.exe`, so
  an agent cannot casually approve its own requests with `beacon decide`. A process that was
  reparented away from the session, or any other process of the same user, can still reach the pipe
  and send the request: the pipe is only closed to other users. Nothing here defends against a
  hostile agent. (Prompts are refused the same way since 2026-11-26.)
- **The wake hook reconnects and exits when its Claude process is gone.** The `StopWaiter` helper
  that outlives a core restart keeps trying to register again, and now ends when the process named
  by `CLAUDE_PID` no longer exists or after 24 hours, instead of waiting forever. The core likewise
  releases a waiter for a session it still does not know after a reconcile pass.

## Pill arrow, a lighter panel, settings and idle wake (2026-11-22)

### The pill's arrow and hover

- **The pill opens its panel from a chevron, not from a click anywhere.** The chevron at the end of
  the pill points down while the panel is closed and turns up while it is open; a press on it
  toggles the panel in sticky mode without taking focus. A press elsewhere on the pill only starts a
  drag, so moving the pill no longer risks opening the panel. The edge tab still toggles on a click.
- **Hover opening is a setting.** `open_on_hover` ("Open panel on hover" under Overlay) is on by
  default for the edge tab and off for the pill; until it is set, the default follows the current
  style.
  With it off, hovering the reveal strip of a hidden overlay only brings the overlay back.
- **The open panel follows the overlay.** While the pill or the tab is dragged, and while the tab
  glides to its dock, the panel is placed against the overlay's current rectangle on every move.
  The overlay reads drag positions from its own mouse messages instead of polling the cursor.

### A lighter panel and core

The core re-sent every session it held, including every ended session from the last 24 hours with
its task list, on every change, and the panel's WebView was built from scratch on each open.

- **A snapshot carries live sessions and the five latest ended ones.** Ended sessions are kept for
  2 hours and at most 20 (was 24 hours with no cap), and the five newest go out in a slim form with
  `ended_count`. The panel fetches the full ended list with the new `ended` request when its Ended
  group is expanded. Task lists come only with the session detail, and empty fields are left out of
  the JSON. Trade-offs: less history is kept, `beacon status` shows only the five latest ended
  sessions, and the Ended group fills in after a request when it is expanded.
- **The panel WebView is kept warm.** It is created hidden when the pointer enters the overlay or
  its reveal strip, and shown without activation when the hover dwell ends. A closed panel is hidden
  instead of destroyed and kept for 45 s (60 s while Claude is active); its page is hidden, so its
  clock and animations stop. The shell hands the page the latest snapshot and UI settings in an
  initialization script and in the opened event, so `get_snapshot` and `get_ui_settings` are gone
  and the page no longer handles missing UI settings. This revises the M2 decision that the panel is
  destroyed when it closes. Trade-off: the M2 measurement put a live WebView2 at about 160 MB shown
  or hidden, so Beacon now holds that memory for up to 60 s after the panel closes, and also after
  the pointer merely passes over the overlay. The warm-panel memory itself was not measured.
- **Rows re-render only when their data changed.** Snapshots reuse the previous session and request
  objects when they are unchanged, rows are memoized with stable callbacks, and the once-a-second
  clock lives in the time cells, runs only while a turn or request is shown, and stops while the
  page is hidden.
- **`claude agents --json` runs on events and every 5 minutes, not every minute.** A pass runs at
  start, when a session's process exits, when a hook names an unknown session and when the panel
  opens (the new `reconcile` request), at most once per 15 s; otherwise every 5 minutes
  (`reconcile_seconds: 300`) while a session is live. The account cache is still checked every
  minute. Trade-off: sessions known only from that list, their listed names and status, and
  sessions that disappeared without a watched process can be up to 5 minutes late when none of
  those events happens.
- **Snapshots are coalesced over 250 ms** (was 100 ms). A snapshot that changes what needs the user,
  a pending decision or an alerting session appearing or going, is still sent at once. The git
  branch is read when the working folder changes and at session start, prompt and stop instead of
  on every event, `state.json` is written at most every 2 s (was 500 ms), and the shell reads the
  sound setting from memory instead of `ui.json` on every snapshot. Trade-offs: other updates can
  arrive up to 250 ms later, a branch switched mid-turn shows at the next prompt or stop, and a
  crash can lose up to 2 s of state.

### Settings, titles, idle wake and clearing

- **Every setting is in one compact settings view.** Overlay style, display, avoided displays and
  position, which were only in the tray menu, are in the settings view too, beside a `Quit Beacon`
  button. Rows use switches, segmented controls and dropdowns with shorter labels, and the six
  sections start collapsed, each with a one-line summary of its choices.
- **The panel can be pinned while Claude is active.** With `pin_while_active` on (off by default),
  the panel opens by itself without focus while a session is starting, working, stalled or needs
  the user, ignores presses outside it, and closes 5 s after Claude is done. Its pin button hides it
  until Claude is next active; it does not open over a fullscreen window. This is an opt-in
  exception to "the panel never holds itself open" from 2026-11-20. (Corrected on 2026-11-26: in
  0.2.0 an unseen finished or failed session also held it open, so it did not close after Claude
  was done, and a fullscreen window taking the foreground did not close it.)
- **Session titles come from the transcript.** A row shows the first of: the custom title (from the
  hooks, or else from the transcript), Claude Code's AI title, the last prompt, the name `claude
  agents` lists (skipped when it is only the project name with a two-digit hex suffix), the project
  name, and the first 8 characters of the id. Titles are redacted and cut to one line of 80
  characters. A session seen only through `claude agents` has its transcript found by id under
  Claude Code's `projects` folder.
- **A prompt sent from Beacon can wake an idle session.** Install now adds a 23rd handler, a second
  `Stop` handler with the argument `StopWaiter`, `asyncRewake` and a 24-hour timeout. The core holds
  it open while the session is idle; a prompt for an idle, done or errored attended session that
  holds one is answered at once, and the helper exits with code 2 and the text "The user sent this
  follow-up from Beacon: ..." on stderr, which starts a new turn. A prompt typed in the session, a
  newer waiter or the session ending releases it; headless runs never get one. This brings back the
  idle wake the v1 scope cut dropped and replaces the VS Code prefill and copy-and-jump paths
  wherever the session can be woken. Spike S6b showed Claude treats the text as an automated
  notification, not user input, so the prompt box says so. Confirmed on a desktop Code tab
  session on 2026-11-22: a prompt sent to the idle session arrived as Stop hook feedback and
  started a turn. **Not yet confirmed in the terminal or VS Code.**
- **Finished sessions clear themselves.** `clear_finished_minutes` in `config.json` (10 by default;
  "Clear finished after" offers Never, 5, 10, 30 minutes and 1 hour) acknowledges a `done` session
  that long after its turn ended, checked with the stall check every 30 s. `error` and `stalled`
  sessions are left for the user.
- `logs\core.log` moves to `logs\core.log.1` before it would pass 5 MB, so the log no longer grows
  without bound. The app's own diagnostics go to a separate `logs\app.log` beside it with the same
  rotation, since a release build has no console.

### Fixes

- **Idle opacity is the overlay's real opacity.** The tab dimmed its working and dormant tones on
  top of the idle opacity, so the default 60% showed as 36%; the tone is now a colour only.
- **The tab does not pulse while the panel is open.** Alerts that arrive while the panel is shown
  count as seen.

## Overlay options, peek and starting with Claude (2026-11-22)

Changed on the owner's feedback: the overlay being on screen all the time was annoying, attention
should be noticeable without being sticky, and Beacon should start with Claude rather than with
Windows.

- **The overlay is not always on screen.** `visibility` is `always`, `active` (a session is working
  or needs the user; the default), `attention` or `never`. It fades out 5 s after its condition
  stops holding. While hidden, a 2 px strip on its screen edge reveals it on hover, so it stays
  within reach. (Superseded: the reveal strip is a 4 px band on the pill's spot, or the tab's 8 px
  footprint. `active` also covers a session that is starting or stalled, or has an alert not yet
  seen.)
- **Attention is a short label, not a held-open panel.** When a session starts needing the user the
  overlay pops, bursts once and slides out a "peek" label naming the session and what it needs, for
  a few seconds. A reminder burst repeats at a chosen interval until the panel is opened. Style,
  interval, label time, size and idle opacity are settings, with a preview. This replaces the burst
  schedule in the section below.
- **Beacon starts when Claude is used.** With `start_mode: claude` the app leaves a marker file, and
  the hook starts the app when an attended session starts or sends a prompt and the core is not
  reachable. Headless runs never start it. The app quits after Claude has been unused for a chosen
  time. The check costs nothing while the core is running: the hook measured 28 ms after against
  31 ms before.
- **The panel is an owned popup.** Some program on the development machine re-centres new unowned
  application windows on the primary display: the panel was created in place and moved by a
  cross-thread `SetWindowPos` from outside Beacon about 190 ms later. Owned popups are left alone, so
  the panel is now owned by the overlay window. Which program does it was not identified.
- **A prompt delivered at turn end is also shown in the session** through the hook's
  `systemMessage`, as "Beacon follow-up: ...". A hook cannot add a real user message.
- Streamlining: one bell menu for snooze and sounds, 36 px rows, the last message and usage under a
  "More" disclosure, and a view's data is loaded before the view slides in so nothing pops in.
  (Superseded: 0.2.0 has 52 px rows in a 376 px panel.)

## Edge tab, compact panel and motion (2026-11-20)

Changed on the owner's feedback after using v1.

- **The panel never holds itself open.** The original plan wanted it to refuse to go away while
  something was waiting; in use that got in the way of other work. An alert no longer opens it, and
  it closes on a press anywhere outside it, on the cursor leaving it, or on losing focus. A
  low-level mouse hook sees the outside press and exists only while the panel does. (Superseded: the
  hook is installed only while the panel is shown in hover or sticky mode, not while it is pinned or
  kept warm.)
- **An edge tab is the default native form.** A strip 4 px thick and 72 px long sits flush against
  a screen edge; hovering opens the panel against that edge and a click keeps it open. The pill
  remains as a second style. A display's default edge is the first one that touches no other
  display, so moving between monitors does not brush it.
- **Attention is the tab's colour and a pulse.** Sessions that started alerting since the panel was
  last opened are "unseen". While any is unseen the tab pulses in bursts: 0.9 s every 3 s for two
  minutes, then every 10 s. A continuous pulse would animate for hours while the user is away.
- **Compact rows.** A session is one 32 px row, with a second 28 px row carrying Allow and Deny when
  it is waiting. Everything else is behind expanding the row. The panel is 340 px wide and 18
  sessions fit where 6 did. (Superseded: 0.2.0 has 52 px rows in a 376 px panel.)
- **Motion follows the display.** In the panel only `transform`, `opacity` and grid-row heights
  animate, as CSS or Web Animations, so the compositor paces them at the refresh rate; measured at
  238 Hz with no slow frames on expand, reorder and view switch. The native tab is paced by a
  high-resolution timer at the display's refresh rate capped at 60 frames per second (never below
  30), computed from elapsed time, and the timer is parked when nothing moves: 0.00 CPU seconds
  over 60 s idle and 0.2 while pulsing.
- **The panel grows instead of scrolling.** Its height follows its content up to the display's work
  area, and nothing inside it scrolls by itself; a block over 40 lines is cut with a "Show all"
  button. When the whole panel does overflow, native scrollbars stay hidden and an overlay thumb and
  edge fades, driven by a CSS scroll timeline, show the position.
- **Sessions have a face, a title and progress** (adapted from a reference widget the owner sent). Each
  session gets an avatar whose colour is fixed by its id and whose expression follows its state,
  with the state mark as a badge. A row leads with the session's title and shows task progress from
  Claude's own task list. Opening a row replaces the list with a detail view: the plan as a
  checklist, each tool call as one step with its status and duration, and the command running now.
- **A session can be hidden** (first called "crossed off"). It leaves the list and the overlay and
  stops alerting until its next prompt or request; Undo, Show in the Hidden group, or
  `beacon undismiss` brings it back. A session with a pending request cannot be hidden. Hiding never
  closes the session: Beacon cannot close a VS Code or desktop session, and leaving it in the
  overlay made the action look like it did nothing.
- **Sounds are optional and off by default:** Windows' own notification sounds when a session starts
  needing the user or finishes.
- Before content grows, the page sizes the window to the target height and then animates inside it,
  so the window never trails its content.

## M3 to M5 and display targeting (2026-11-20)

### Scope cut for v1

Asked to drop what is not needed: the VS Code companion extension, Antigravity support, the claude.ai
browser extension, the desktop-chat adapter, Beacon-spawned sessions, idle wake, headless resume, the
edge-reveal form, toast action buttons and the test-matrix document are all out of v1. (Idle wake
returned on 2026-11-22.)

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

(Superseded on 2026-11-22: an idle session is woken through the `StopWaiter` hook; prefill and copy
and jump are the fallback for a session that cannot be woken. Prefill now also puts the text on
the clipboard.)

- A queued prompt shows `queued`, `delivered`, `acknowledged` or `failed` with a reason, and
  survives a core restart. A turn that ends without a `Stop` fails its queued prompts instead of
  leaving them queued.
- A `Stop` that delivers a prompt is not the end of the turn.
- Verified against real Claude Code: a prompt queued during a running command was delivered at
  `Stop` and Claude answered it.

### Display targeting

- `ui.json` holds the chosen display and a list of displays to avoid. The pill and the panel appear
  only on the target: the chosen display if connected and allowed, otherwise the primary if allowed,
  otherwise the first allowed display, otherwise nowhere (tray only). (Superseded: with no allowed
  display the overlay stays hidden, but the panel opens on the primary display.)
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
  (Superseded: 0.2.0 ships a per-user NSIS installer.)

### Build

`[profile.fast]` (no link-time optimisation) is for day-to-day and verification builds. The app must
be built through the Tauri CLI, or it loads the dev server instead of the bundled front end.

## M2: tray, pill and panel (2026-11-20)

### The pill is native; the panel is a WebView that exists only while it is open

Measured on the development machine with a release build, 60 s of sampling after a 60 s warm-up:

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
takes about 0.3 s. The app is over the idle budget only while the panel is open. (Superseded on
2026-11-22: a closed panel is hidden and kept warm for 45 s, or 60 s while Claude is active, before
it is destroyed.)

### Pill

- One mark per session, at most ten and then `+N`, each state with its own shape as well as colour:
  working is a rotating arc, the four needs-input states a solid amber circle with a halo, stalled a
  pause glyph, done a green check, error a red triangle, idle a hollow ring. A count of the sessions
  that need the user follows the marks.
- The animation timer runs only while something on the pill animates and reduced motion is off.
- Fully transparent pixels pass clicks through. The pill never takes focus.
- It hides while a fullscreen window is in the foreground on its monitor. That is driven by the
  foreground-change hook, so a window that goes fullscreen while already in the foreground is not
  noticed until the foreground changes. (Superseded: a location-change hook on the foreground
  window now notices it going fullscreen.)
- Its position is stored per monitor layout in `ui.json` and snaps to edges and corners.

### Panel

- Two modes. Hover closes 400 ms after the cursor leaves the tab or pill and the panel. Sticky (a
  click, the hotkey, the tray, a second launch) stays until dismissed. The pinned mode this
  milestone shipped with was removed; see "Edge tab, compact panel and motion". (An opt-in pinned
  mode returned on 2026-11-22.)
- A hover open does not take focus.
- The page can call a fixed list of commands and nothing else, restricted to the panel window by a
  capability file. The CSP is `default-src 'self'` plus the `connect-src` Tauri's own IPC needs.
- Everything from a session is rendered as text nodes. The last message goes through a hand-written
  renderer for a Markdown subset; there is no HTML injection path and no Markdown dependency.
- The panel is a solid Graphite surface. The original plan's OS blur is not used, because Graphite
  is flat.

### Attention, acknowledge and snooze

- `alerting` means in an attention state and not snoozed. The tab's colour and pulse, the pill count,
  the tray icon and toasts all follow it.
- Acknowledge clears `done`, `error` and `stalled`. The original plan had it only for `done`, which
  would leave an API error pinning the panel until the next prompt.
- Snooze is per session or global, for 5 or 15 minutes, held in the core so the CLI and the app
  agree, and it survives a restart. (Superseded: the tray also offers 1 hour, and `beacon snooze`
  takes any number of minutes.)

### Toasts

- The first snapshot after start is a baseline. Sessions that start alerting or finish within 2 s of
  each other become one toast. Snoozed sessions produce none.
- Toasts have no action buttons; those need the packaged app identity.

### Hotkey

`Ctrl+Alt+Space` is already registered by another program on the development machine, so it never
fired there. The accelerator is read from `hotkey` in `ui.json` and can be changed in the settings
view.

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
- A light theme is not planned, because Graphite is dark only. (Superseded: 0.2.0 follows the
  Windows theme, light or dark.)

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
  deadline, because the wait is for the person. (Corrected on 2026-11-26: an MCP form now gets the
  same 1 s reply budget, because the helper holding it hides Claude Code's own dialog; only
  permission requests wait without a deadline.)
- **The 15 ms budget is not met: an event costs about 30 ms.** That is process creation on the
  development machine; an empty run of the helper takes 31 ms and `whoami.exe` 34 ms. It delays a session only on
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
  (Superseded: an unreadable `state.json` is kept as `state.json.bad` and the core starts with no
  sessions; only a malformed `config.json` stops the core.)
- Ended sessions are dropped from the registry 24 hours after their last event. (Superseded on
  2026-11-22: they are kept for 2 hours from when they ended, and at most 20.)
- A session known only from `claude agents --json` is marked limited until one of its hooks fires.
- The end-to-end check ran against a real `claude -p` session with a temporary settings file: allow
  from `beacon decide` ran the tool, deny returned the message to Claude, and uninstall restored the
  file byte for byte.

## Phase 0 (2026-11-19)

### Where the original plan was wrong or incomplete

- **`PermissionRequest` has no `tool_use_id`.** The original plan keyed pending decisions on
  `session_id` + `tool_use_id`. A pending decision is keyed on the helper connection instead, and its `tool_use_id`
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

### Hook registration, changed from the original plan's table

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

### Usage metrics (added to the original plan on 2026-11-19)

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
  as Graphite is, which replaces the original plan's follow-the-OS light and dark. `ink` is the
  accent; `wip` marks needs-input, `live` done, `danger` error. (Superseded: 0.2.0 follows the
  Windows theme, light or dark.)
- **React 19, TypeScript, Vite and Tailwind 4** for the UI, as in every other Graphite app. The size
  difference from Svelte is noise next to WebView2.
- **Tauri 2 with a Rust core, and a native Rust helper.** A Node helper measured 46 ms per event
  against the 15 ms budget. Visual Studio Build Tools 2026 with the C++ tools and WebView2 157 are
  installed on the development machine; Rust is not. **Open:** approval to install the Rust
  toolchain with rustup. (Done: Rust was installed and M1 was built with it.)
- **The long-running process is called the core, not the hub** (`crates/core`), because Graphite Hub
  already exists.
- **Risk: the 80 MB idle budget.** A WebView2 window that is always on screen may not fit. It is
  measured first in M2; the fallback is a natively drawn collapsed pill with the WebView created on
  expand.

### Deferred

- `.graphite.yml` is added in M1, once the stack is approved and the tags are known. (Done.)
- Antigravity is not installed on the development machine, so S8 is **open**.
- The interactive terminal, VS Code and desktop surfaces are unmeasured until the manual probe in
  SPIKES.md is run. They gate M4 (jump to window, surface icons), not M1. (The desktop Code tab has
  since been verified; the terminal and VS Code have not.)
