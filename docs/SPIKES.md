# Beacon phase 0 spike results

These results were measured on 2026-11-19 against Claude Code 2.1.328. They are kept as a record of
what the phase 0 experiments found and are not maintained; [DECISIONS.md](DECISIONS.md) says what
was built from them and what changed since.

- Date: 2026-11-19
- Claude Code: 2.1.328
- OS: Windows 11 Pro
- Model for every run: `haiku` (`claude-haiku-5-5`; plan-mode turns ran on `claude-sonnet-5-5`, see S5)
- Every run was `claude -p` with `--output-format stream-json --verbose --include-hook-events`, started
  by `spikes/harness.mjs` with all `CLAUDE*` and `ANTHROPIC*` variables removed from its environment.
  The hook payloads and stream report entrypoint `sdk-cli`. Interactive terminal, VS Code extension
  and desktop Code tab behaviour is NOT measured here.
- Run folders are `spikes/out/runs/<scenario>/` (git-ignored): `stream.jsonl`, `events.jsonl`,
  `timeline.txt`, `control_requests.jsonl`, `heartbeat-<pid>.txt`, `meta.json`.

## Status

| Spike | Status | Result |
|---|---|---|
| S1 event coverage | partly confirmed | 10 of the 28 registered events fired in `-p`; the other 18 were not triggered by these prompts. |
| S2 permission round trip | confirmed | `behavior`, `message`, `interrupt`, `updatedPermissions` and `updatedInput` all took effect from a PermissionRequest hook. |
| S3 race with the native prompt | partly confirmed | Against a stdio host, the first decision wins and Claude Code sends `control_cancel_request` to the host; the hook is never cancelled. Terminal and GUI prompts not exercised. |
| S4 timeout | confirmed | A hook past its `timeout` is killed at about 5.2 s, reported as `cancelled`, and the call falls through to the default decision. |
| S5 questions and plans | partly confirmed | Works only with `--permission-prompt-tool stdio`; plain `-p` does not offer AskUserQuestion or ExitPlanMode. Elicitation not run. |
| S6 prompt injection | partly confirmed | Stop block, Stop additionalContext, asyncRewake, live resume and streaming input work; terminal injection not testable here; the inbox pipe delivered nothing in 3 attempts. |
| S7 window identity | partly confirmed | Env and ancestor chain captured for `-p` only; no terminal or editor variables were present. |
| S8 Antigravity | open | Not installed on the development machine. |
| S9 usage metrics | partly confirmed | Per-session tokens and estimated cost are readable for every surface; the 5-hour and weekly limit % have no documented source that covers every surface. |

## S1 event coverage

**Question.** Which hook events fire, in what order, and with which payload keys?

**What was run.** `s1-observe` (observe-only logger on all 28 events), plus every other scenario,
which all carry the same logger.

**Observed.** Order in `s1-observe`: InstructionsLoaded, SessionStart, UserPromptSubmit, PreToolUse,
PermissionRequest, PostToolBatch, Stop, SessionEnd. A `-p` manual-mode permission denial fires
neither PermissionDenied nor PostToolUseFailure; it shows in the stream as
`{"type":"system","subtype":"permission_denied",...,"decision_reason":"This command requires approval"}`.

| Event | Top-level payload keys | tool_use_id | permission_mode | agent_id |
|---|---|---|---|---|
| InstructionsLoaded | session_id, transcript_path, cwd, hook_event_name, file_path, memory_type, load_reason | n | n | n |
| SessionStart | session_id, transcript_path, cwd, hook_event_name, source | n | n | n |
| UserPromptSubmit | session_id, transcript_path, cwd, prompt_id, permission_mode, hook_event_name, prompt | n | y | n |
| PreToolUse | session_id, transcript_path, cwd, prompt_id, permission_mode, effort, hook_event_name, tool_name, tool_input, tool_use_id | y | y | n |
| PermissionRequest | session_id, transcript_path, cwd, prompt_id, permission_mode, effort, hook_event_name, tool_name, tool_input, permission_suggestions | n | y | n |
| PostToolUse | PreToolUse keys plus tool_response, duration_ms | y | y | n |
| PostToolBatch | session_id, transcript_path, cwd, prompt_id, permission_mode, effort, hook_event_name, tool_calls (each with tool_use_id) | n | y | n |
| Stop | session_id, transcript_path, cwd, prompt_id, permission_mode, effort, hook_event_name, stop_hook_active, last_assistant_message, background_tasks, session_crons | n | y | n |
| SessionEnd | session_id, transcript_path, cwd, prompt_id, hook_event_name, reason (`other`) | n | n | n |
| PostModelSwitch | session_id, transcript_path, cwd, prompt_id, hook_event_name, from_model, to_model, requested_model, source, context_tokens, prompt_cache_warm, cache_ttl, estimated_cache_write_usd, pricing | n | n | n |

Sanitized samples are in `fixtures/hook-payloads/<EventName>.json`.

**Not covered.** Not triggered by any prompt: Setup, UserPromptExpansion, PermissionDenied,
PostToolUseFailure, Notification, SubagentStart, SubagentStop, TaskCreated, TaskCompleted,
StopFailure, TeammateIdle, ConfigChange, CwdChanged, DirectoryAdded, PreCompact, PostCompact,
Elicitation, ElicitationResult. `agent_id` never appeared because no subagent ran.

## S2 permission round trip

**Question.** Can a PermissionRequest hook hold the call and return allow or deny with the documented
fields?

**What was run.** `s2-allow`, `s2-deny`, `s2-interrupt`, `s2-always`; `updatedInput` from
`s5b-ask-pre`, `s5b-ask-perm`, `s5b-plan`.

**Observed.**

- `behavior: allow` after a 20 s hold: hook started 15:16:31.307, emitted 15:16:51.320, PostToolUse
  15:16:52.040, tool_result `"beacon-spike-ok"`.
- `behavior: deny` with `message`: the tool_result is the message verbatim,
  `"Denied from Beacon spike: do not retry, just say DENIED."`, and the stream line is
  `permission_denied` with `"decision_reason_type":"hook"`. The model answered DENIED.
- `interrupt: true`: the tool_result becomes the standard rejection text, followed by a user text
  `[Request interrupted by user for tool use]`. Result
  `{"subtype":"error_during_execution","is_error":true,"stop_reason":"tool_use","terminal_reason":"aborted_tools"}`,
  exit code 1, no Stop and no PostToolBatch.
- `updatedPermissions` with an `addRules` entry (`Bash`, `node -e *`, destination `session`): the
  second matching Bash call ran without a second PermissionRequest (PreToolUse 15:17:27.782,
  PostToolUse 15:17:28.196).
- `updatedInput`: the AskUserQuestion answers injected through `updatedInput` reached the model from
  both PreToolUse and PermissionRequest (see S5).

**Not covered.** `updatedPermissions` destinations other than `session`; the `setMode` update type.

## S3 race with the native prompt

**Question.** When a PermissionRequest hook and another approver are both pending, which decision wins,
and what happens to the loser?

**What was run.** `s3-host-race`, `s3-hook-first`, `s3-host-deny`, all with
`--permission-prompt-tool stdio` and `--input-format stream-json`, the harness acting as the host.

**Observed.**

- The host `control_request` and the PermissionRequest hook start together: in `s3-host-race` the
  request reached the host at 15:20:42.285 and the hook logged its start at 15:20:42.338. The request
  carries `tool_use_id`; the PermissionRequest hook payload does not.

  ```json
  {"type":"control_request","request_id":"7214c433-...","request":{"subtype":"can_use_tool","tool_name":"Bash","display_name":"Bash","input":{...},"description":"...","permission_suggestions":[...],"decision_reason":"This command requires approval","decision_reason_type":"other","tool_use_id":"toolu_01EFxHE15RzeEaWjrt5Z7CgR"}}
  ```

- Host first (`s3-host-race`): host allow at 15:20:45.293, PostToolUse 15:20:45.931. The pending
  hook was not killed or signalled; it emitted at 15:21:02.354, exited 0, and Claude Code still
  printed a `hook_response ... success` for it after the result, with no visible effect.
- Hook first (`s3-hook-first`): the hook allowed at 15:29:55.483 and Claude Code then sent the host
  `{"type":"control_cancel_request","request_id":"1488898d-ecb7-4eb9-830c-6a9fa21eafb0"}` at
  15:29:55.496. PostToolUse followed at 15:29:56.145.
- Host denies (`s3-host-deny`, second attempt; in the first attempt the model replied without calling
  the tool): host deny at 15:30:29.884. The tool_result was the host message,
  `"Denied by host in spike; just say DENIED."`, is_error true. The hook events after the denial were
  PostToolBatch (15:30:29.943, its `tool_calls[0].tool_response` is the deny message) and Stop
  (15:30:32.334). There was no PostToolUse, PostToolUseFailure or PermissionDenied, and no
  `permission_denied` stream line. The result's `permission_denials` lists the call. The pending hook
  ran on to 15:30:46.946 and exited 0.
- PostToolUse carries the same `tool_use_id` as the matching PreToolUse in every run.

**Not covered.** The terminal TUI permission prompt and the real VS Code and desktop permission
dialogs were not exercised; only the stdio host protocol was.

## S4 timeout

**Question.** What happens when a PermissionRequest hook outlives its `timeout`?

**What was run.** `s4-timeout`: `"timeout": 5`, hook delay 30 s.

**Observed.** The hook started at 15:17:36.988 and its last heartbeat was 15:17:42.218 (5.23 s).
It logged no emit, no exit and no signal, so the process was terminated without running its
handlers. Stream: `hook_response PermissionRequest:Bash exit_code 1 outcome "cancelled"`, then
`permission_denied` with `decision_reason_type "other"` and `"This command requires approval"`. The
turn ended normally with subtype `success`.

**Not covered.** Timeout behaviour with a stdio host or an interactive prompt present.

## S5 questions and plans

**Question.** Can a hook answer AskUserQuestion and approve ExitPlanMode?

**What was run.** Plain `-p`: `s5-ask-pre`, `s5-ask-perm`, `s5-plan`, `s5-plan-pre`. With
`--permission-prompt-tool stdio` and a host that never answers: `s5b-ask-pre`, `s5b-ask-perm`,
`s5b-plan`.

**Observed.**

- Plain `-p`: the `system/init` tools list has neither AskUserQuestion nor ExitPlanMode. A direct
  ExitPlanMode call returned `No such tool available: ExitPlanMode. ExitPlanMode is disabled for this
  session, in subagents as well as here.` No hook fired for either tool. In plan mode the model wrote
  plan files under `%USERPROFILE%\.claude\plans\`.
- With the stdio host, both tools are in the init tools list.
- `s5b-ask-pre`: PreToolUse fired for AskUserQuestion (15:28:25.240); the hook returned
  `permissionDecision: allow` with `updatedInput: {questions, answers}`. No PermissionRequest and no
  `control_request` followed. tool_result:
  `Your questions have been answered: "Which colour?"="Red". You can now continue with these answers in mind.`
  The model replied that Red was picked.
- `s5b-ask-perm`: PermissionRequest fired for AskUserQuestion, and the host received
  `{"type":"control_request",...,"request":{"subtype":"can_use_tool","tool_name":"AskUserQuestion","display_name":"AskUserQuestion","input":{"questions":[...]},"tool_use_id":"toolu_01Pdkn4tPwNRcLZmBzFVEvUx","requires_user_interaction":true}}`.
  The hook decided at 15:28:38.720 and `control_cancel_request` reached the host at 15:28:38.733.
  The tool_result text was identical to `s5b-ask-pre`.
- `s5b-plan`: ExitPlanMode `tool_input` was `{}` in PreToolUse, in PermissionRequest and in the
  `control_request` input, although the model's tool_use block carried `plan`. The control request
  also had `requires_user_interaction: true`. Approval through the hook worked: tool_result
  `User has approved exiting plan mode. You can now proceed.`, stream
  `{"type":"system","subtype":"status","status":null,"permissionMode":"default",...}`, and the model
  answered APPROVED. `permission_mode` was `plan` up to PermissionRequest and `default` from the
  ExitPlanMode PostToolUse onwards. That PostToolUse `tool_response` was
  `{"plan":null,"isAgent":false,"filePath":"...\\.claude\\plans\\plan-how-to-add-zazzy-bonbon.md"}`;
  that file was not created. PostModelSwitch fired with `from_model claude-sonnet-5-5`,
  `to_model claude-haiku-5-5`, `source auto`: the plan-mode turns ran on Sonnet although the run
  asked for `--model haiku`.

**Not covered.** Elicitation was not run; its documented contract is in the hooks reference
(`hooks.md` lines 3710-3804: matcher on MCP server name, input `mcp_server_name`, `message`, optional
`mode`, `url`, `elicitation_id`, `requested_schema`). Multi-question and multi-select
AskUserQuestion; plan rejection.

## S6 prompt injection paths

**Question.** Which paths can deliver a follow-up from Beacon into a session, and how is it framed?

**What was run.** `s6a-stop`, `s6a-context`, `s6b-rewake`, `s6c-resume-live`, `s6d-stream`,
`s6f-inbox` (3 attempts).

**Observed.**

a) Stop hook.

- `decision: block` with `reason` (`s6a-stop`): the stream shows a user message
  `"Stop hook feedback:\nThe user sent this follow-up from Beacon: reply with exactly the single word PINEAPPLE and nothing else."`
  and a notification
  `{"type":"system","subtype":"notification","key":"stop-hook-error","text":"stop hook reported a problem · ctrl+o to see",...}`.
  The model answered PINEAPPLE; the next Stop had `stop_hook_active: true`.
- `hookSpecificOutput.additionalContext` (`s6a-context`): no user line and no `stop-hook-error`
  notification in the stream. The transcript stores it as an attachment of type
  `hook_additional_context`, rendered to the model as
  `<system-reminder>\nStop hook additional context: The user sent this follow-up from Beacon: ...\n</system-reminder>`.
  The model answered PAPAYA, the result text was `"PAPAYA"` with `num_turns: 1`, and the next Stop
  had `stop_hook_active: true`.

b) asyncRewake (`s6b-rewake`). With stdin open and the session idle after the first result
(15:19:43.884), the Stop hook exited with code 2 at 15:19:55.715 and a new turn started by itself:
UserPromptSubmit at 15:19:55.787, the model said MANGO, second result at 15:19:59.835. In the stream
only the `hook_response` (`exit_code 2`, `outcome "error"`, `stderr` = the text) carries it; no user
line is emitted. The UserPromptSubmit `prompt` and the transcript user message are exactly:

```text
<task-notification>
<summary>Stop hook feedback</summary>
</task-notification>
<system-reminder>
Stop hook blocking error from command "Stop": The user sent this follow-up from Beacon: reply with exactly the single word MANGO.
</system-reminder>
```

The transcript entry has `origin.kind "task-notification"`, `promptSource "system"` and
`turnOrigin "task_notification"`. The request sent to the model wraps it in a `<system-reminder>`
that starts `[SYSTEM NOTIFICATION - NOT USER INPUT]` and states it is an automated background-task
event, not a message from the user. The Stop after the rewake turn had `stop_hook_active: true`.

c) Resume against a live session (`s6c-resume-live`). While session A was open, a second process
`claude -p --resume <A> --model haiku --output-format json "Reply with the single word BRAVO."`
exited 0 and reported the same `session_id` (`c7e90ea3-...`). A's transcript grew from 31 to 51
lines and contains BRAVO; no new transcript file appeared. Asked afterwards for the last word, A
answered ALPHA: A did not see the BRAVO turn.

d) Streaming input (`s6d-stream`). Two user lines on stdin gave two results with the same
`session_id`; UserPromptSubmit fired for each (15:19:22.834 and 15:19:24.491). A `system/init` line
is emitted before each turn; SessionStart fired once.

e) Terminal injection. `tmux`, `wezterm` and `kitty` are not on PATH on the development machine;
not tested.

f) Inbox pipe (`s6f-inbox`). A SessionStart hook captured `CLAUDE_CODE_MESSAGING_SOCKET`
(`\\.\pipe\LOCAL\cc-msg-<32 hex>`) and the 32-character token; the harness connected to the pipe
after the idle READY turn and sent the documented auth line `{"type":"auth","token":"<token>"}`.

- Attempt 1: auth line only.
- Attempt 2: auth line, then `{}`.
- Attempt 3: auth line, then a plain-text line asking for KIWI (the docs say cross-session messages
  are plain text).

In all three the pipe accepted the connection, sent nothing back, and did not close it before the
harness ended it 5 s after the last line. No turn started and KIWI appears in neither the stream
nor the transcript. The docs describe only the auth line; the message envelope that follows it is not
documented and was not discoverable in 3 attempts.

**Not covered.** `crossSessionInbound: accept` in `--settings` was not tried for the inbox.
Injection into interactive terminal, VS Code or desktop sessions.

## S7 window identity

**Question.** What can a hook process learn about the window or host it runs under?

**What was run.** `s1-observe` (SessionStart and PermissionRequest record the ancestor chain).

**Observed.** Variables matching the logger's filter, as seen by the hook in a `-p` run:

- `CLAUDECODE=1`
- `CLAUDE_CODE_CHILD_NOTES_DIR=<TEMP>\claude\<hash>\<session_id>\started`
- `CLAUDE_CODE_CHILD_SESSION=1`
- `CLAUDE_CODE_ENTRYPOINT=sdk-cli`
- `CLAUDE_CODE_MESSAGING_SOCKET=\\.\pipe\LOCAL\cc-msg-<32 hex>`
- `CLAUDE_CODE_MESSAGING_TOKEN` (32 characters, redacted)
- `CLAUDE_CODE_SESSION_ATTENDED=0`
- `CLAUDE_CODE_SESSION_ID=<session_id>`
- `CLAUDE_CODE_SUBAGENT_MODEL=sonnet` (source not checked)
- `CLAUDE_PID=51388`, the pid of the `claude.exe` that spawned the hook
- `CLAUDE_PROJECT_DIR=<work dir>`
- `SESSIONNAME=Console`

No `TERM`, `WT_*`, `VSCODE*`, `TMUX`, `WEZTERM*` or other terminal variables were present. Ancestor
chain from the hook upwards:
`node.exe (hook) -> claude.exe (51388) -> node.exe (harness) -> pwsh.exe -> cmd.exe -> claude.exe -> claude.exe`.

**Not covered.** Hooks under Windows Terminal, the VS Code extension and the desktop Code tab.

## S8 Antigravity

**Question.** Does Beacon's approach work inside Antigravity?

**What was run.** PATH and folder checks only.

**Observed.** `antigravity`, `cursor` and `windsurf` do not resolve on PATH; there is no Antigravity
folder in `%LOCALAPPDATA%\Programs` and no `%USERPROFILE%\.antigravity` or
`.antigravity\extensions`.

**Not covered.** Everything: Antigravity is not installed on the development machine, so this was
not testable there.

## S9 usage metrics

**Question.** Which local sources give per-session usage and overall account usage (5-hour and weekly
limit %, reset times, spend), and on which surfaces?

**What was run.** `spikes/usage-probe.mjs` over every transcript modified in the last 24 hours (39
sessions, 372 MB), `spikes/otel-probe.mjs` (a loopback OTLP `http/json` listener plus one `claude -p`
run), one `--output-format json` run and one `--output-format stream-json --verbose` run, signed
in with a claude.ai subscription.

**Observed.**

- Transcripts (`~/.claude/projects/<project>/<session>.jsonl`, internal format).
  - Every assistant line carries `message.usage` with `input_tokens`, `output_tokens`,
    `cache_creation_input_tokens`, `cache_read_input_tokens`. Lines also carry `entrypoint`; the
    values seen were `claude-desktop`, `claude-vscode` and `sdk-cli` (no terminal session in the
    sample).
  - The same `message.id` repeats once per content block with identical usage (0 ids with differing
    usage). A naive sum overcounts 2.5 to 4 times, for example output 2,942,840 naive against 924,077
    after keeping one usage per `message.id`.
  - Subagent usage is not in the main file; it is in `<session>/subagents/*.jsonl` (one session had
    88 such files).
  - Context fill: the last assistant usage (`input_tokens + cache_creation_input_tokens +
    cache_read_input_tokens`) is the current context size; the window size is not in the transcript.
  - A `cost-state` line with `totalCostUSD` and per-model `costUSD` was in 33 of 39 transcripts. It is
    written when a session closes or clears and was absent from the 3 sessions still running.
  - Rate-limit data appears only on HTTP 429 error lines (`quotaLimits`,
    `apiErrorParams.rate_limit_info`).
  - A full scan of all 39 files took 1.1 s.
- OpenTelemetry. With `CLAUDE_CODE_ENABLE_TELEMETRY=1`, `OTEL_METRICS_EXPORTER=otlp`,
  `OTEL_LOGS_EXPORTER=otlp`, `OTEL_EXPORTER_OTLP_PROTOCOL=http/json` and
  `OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:<port>`, the run posted to `/v1/metrics` and
  `/v1/logs`. Metrics seen: `session.count`, `cost.usage` (USD), `token.usage` (by `type`),
  `active_time.total`. Events seen include `api_request` with `input_tokens`, `output_tokens`,
  `cache_read_tokens`, `cache_creation_tokens`, `cost_usd`. Attributes include `session.id` and
  `app.entrypoint`, and also the account email, account id and organization id. No rate-limit or
  quota attribute appeared. It worked with the claude.ai subscription login.
- `-p` result JSON: `total_cost_usd`, `usage`, and `modelUsage.<model>` with `costUSD`,
  `contextWindow` (1,000,000 for haiku), `maxOutputTokens`, `costBasis: "list"`. No rate-limit field.
- `-p` stream: a `rate_limit_event` was emitted with status `allowed`:
  `"unifiedWindows":{"five_hour":{"utilization":0.47,"resetsAt":1795104600},"seven_day":{"utilization":0.11,"resetsAt":1795690800}}`.
  `SDKRateLimitEvent` is documented; the `unifiedWindows` field is not in the documented type. One
  such run cost about $0.004 at list price.
- `~/.claude.json` has `cachedUsageUtilization` (internal) with `fetchedAtMs`,
  `utilization.five_hour` and `utilization.seven_day` (`utilization` 0 to 100, `resets_at`),
  `limits[]`, `extra_usage` and `spend`. It was fetched at 11:14Z and unchanged at 15:20Z; three `-p`
  runs did not refresh it.
- Statusline. The docs list `rate_limits.five_hour` and `rate_limits.seven_day` but say the field
  "appears only for claude.ai Pro, Max, and Ultra subscribers", and do not say which surfaces run a
  statusline command. The user settings have no `statusLine` key.
- `~/.claude/stats-cache.json`, which the docs describe as the `/usage` aggregate, does not exist on
  the development machine.
- Documented statements on cost: the session dollar figure is computed "locally from token counts at
  list price", "isn't relevant for billing purposes" on subscriptions, and on Team "usage inside the
  seat allowance isn't metered in dollars".

**Not covered.** Statusline on any surface; OpenTelemetry from VS Code or desktop sessions; the mods
`turn.complete` usage event; `SDKUsageReport`; whether VS Code or desktop activity refreshes
`cachedUsageUtilization`; a terminal-session transcript.

## Other measurements

- Hook overhead.
  - `node spikes/hook-logger.mjs` fed a small Stop payload (spawnSync, 10 runs, twice): median 47 ms
    and 46 ms. A bare `node -e 0` has a median of 40 ms.
  - The same logger with the ancestor-chain query (one `powershell.exe Get-CimInstance` call):
    median 628 ms over 5 runs.
  - Claude Code's own `stop_hook_summary` durations for the logger: 56, 69 and 74 ms.
  - PreToolUse `mono` to the matching PostToolUse: median 308 ms over 11 pairs, tool run time
    included; 252 to 415 ms for pairs with no permission wait.
- `claude agents --json`, run while `s6d-stream` was open: an array of entries with `pid`, `cwd`,
  `kind`, `startedAt`, `sessionId`, `name`, `status`. It listed the spike's `-p` stream-json session
  as `kind: interactive`, `status: idle`.
- `%USERPROFILE%\.claude\sessions\<pid>.json` (internal, undocumented) has the keys `pid`,
  `sessionId`, `cwd`, `startedAt`, `procStart`, `version`, `peerProtocol`, `peerFeatures`, `kind`,
  `entrypoint`, `hostSessionId` (only in some files), `pidDomain`, `asksForConvertedSkills`,
  `messagingSocketPath`, `name`, `nameSource`, `nameSince`, `status`, `updatedAt`,
  `statusUpdatedAt`. The folder also holds `.key` files with `peerToken`, `procStartFt`, `pidDomain`.
- The user settings also applied to every spike session: a third-party hook ran on every event next
  to the spike hooks, and a user-level CLAUDE.md was loaded (InstructionsLoaded `memory_type: User`).

## Manual steps still needed (historical)

This list is as it stood on 2026-11-19. Since then the desktop Code tab has been verified end to
end; the terminal, VS Code and Antigravity steps below have not been done.

1. Interactive surfaces.
   1. Run `node spikes/harness.mjs probe <folder>` for an empty folder. It writes
      `<folder>/.claude/settings.json` with the observe-only logger on every event, logging to
      `spikes/out/probe/events.jsonl`.
   2. Open that folder once in each of: Windows Terminal (`claude`), the VS Code extension and the
      desktop Code tab.
   3. In each, ask for the P1 command:
      `Use the Bash tool to run exactly this command: node -e "console.log('beacon-spike-ok')" . Then reply with the command output only. Do not use any other tool.`
   4. Answer the native permission prompt with allow.
   5. Send back from `spikes/out/probe/events.jsonl`: the event order, the `env` and `chain` of the
      SessionStart and PermissionRequest records, and `permission_mode` per event, for each surface.
2. Terminal native-prompt race for S3.
   1. Add a PermissionRequest hook that waits 20 s and then allows (the `s2-allow` decide hook) to
      the probe folder's settings, or run `node spikes/harness.mjs settings s2-allow` and start
      `claude --settings <that file>` in Windows Terminal from `spikes/out/work`.
   2. Ask for the P1 command and answer the terminal prompt within about 3 s.
   3. Record whether the tool runs at once, whether the prompt disappears when the hook decides
      first (repeat with an answer after 25 s), and what `spikes/out/events.jsonl` and the
      `spikes/out/heartbeat-<pid>.txt` file show for the hook process (without `BEACON_SPIKE_OUT`
      the hooks log to `spikes/out`).
3. Antigravity.
   1. Install Antigravity and the Claude Code extension on a machine where that is allowed.
   2. Repeat step 1 inside Antigravity and send back the same facts, including any `ANTIGRAVITY*`
      variables and the ancestor chain.

## How to rerun

```powershell
node spikes/harness.mjs list
node spikes/harness.mjs auth
node spikes/harness.mjs settings <scenario>
node spikes/harness.mjs run <scenario>
node spikes/harness.mjs run s6f-inbox <file with a JSON array of lines to send after the auth line>
node spikes/harness.mjs probe <folder>
node spikes/harness.mjs fixtures
```

`run` writes `spikes/out/runs/<scenario>/`, replacing any earlier run of that scenario, and stops a
run after 240 s.
