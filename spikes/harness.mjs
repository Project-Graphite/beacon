import { spawn } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { connect } from 'node:net';
import { createInterface } from 'node:readline';
import { homedir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { spikesDir } from './spike-log.mjs';

const loggerEvents = [
  'SessionStart', 'Setup', 'InstructionsLoaded', 'UserPromptSubmit', 'UserPromptExpansion', 'PreToolUse',
  'PermissionRequest', 'PermissionDenied', 'PostToolUse', 'PostToolUseFailure', 'PostToolBatch', 'Notification',
  'SubagentStart', 'SubagentStop', 'TaskCreated', 'TaskCompleted', 'Stop', 'StopFailure', 'TeammateIdle',
  'ConfigChange', 'CwdChanged', 'DirectoryAdded', 'PreCompact', 'PostCompact', 'PostModelSwitch', 'Elicitation',
  'ElicitationResult', 'SessionEnd',
];

const outRoot = join(spikesDir, 'out');
const workDir = join(outRoot, 'work');
const hardKillMs = 240000;

const bashPrompt = `Use the Bash tool to run exactly this command: node -e "console.log('beacon-spike-ok')" . Then reply with the command output only. Do not use any other tool.`;
const askPrompt = 'Use the AskUserQuestion tool to ask me one question: "Which colour?" with header "Colour" and options "Red" and "Blue". Then tell me which one I picked.';
const allowPermissionRequest = { hookSpecificOutput: { hookEventName: 'PermissionRequest', decision: { behavior: 'allow' } } };
const deniedMessage = 'Denied from Beacon spike: do not retry, just say DENIED.';

const planPrompt = 'Plan how to add a line "hello" to notes.txt, then present the plan with ExitPlanMode. If the plan is approved, do nothing further and say APPROVED.';
const silentPlanPrompt = `${planPrompt} Do not write, create or edit any file, including a plan file; put the plan only in the ExitPlanMode call.`;
const stdioHost = ['--permission-prompt-tool', 'stdio'];

const isResult = (line) => line.type === 'result';
const sleep = (ms) => new Promise((resolveSleep) => setTimeout(resolveSleep, ms));

const scenarios = {
  's1-observe': { prompt: bashPrompt },
  's2-allow': {
    prompt: bashPrompt,
    decide: [{ event: 'PermissionRequest', scenario: { delayMs: 20000, output: allowPermissionRequest } }],
  },
  's2-deny': {
    prompt: bashPrompt,
    decide: [{
      event: 'PermissionRequest',
      scenario: { delayMs: 2000, output: { hookSpecificOutput: { hookEventName: 'PermissionRequest', decision: { behavior: 'deny', message: deniedMessage } } } },
    }],
  },
  's2-interrupt': {
    prompt: bashPrompt,
    decide: [{
      event: 'PermissionRequest',
      scenario: { delayMs: 2000, output: { hookSpecificOutput: { hookEventName: 'PermissionRequest', decision: { behavior: 'deny', message: deniedMessage, interrupt: true } } } },
    }],
  },
  's2-always': {
    prompt: `${bashPrompt} After that, run this second command with the same tool: node -e "console.log('second')" and report both outputs.`,
    decide: [{
      event: 'PermissionRequest',
      scenario: {
        delayMs: 2000,
        output: {
          hookSpecificOutput: {
            hookEventName: 'PermissionRequest',
            decision: {
              behavior: 'allow',
              updatedPermissions: [{ type: 'addRules', rules: [{ toolName: 'Bash', ruleContent: 'node -e *' }], behavior: 'allow', destination: 'session' }],
            },
          },
        },
      },
    }],
  },
  's4-timeout': {
    prompt: bashPrompt,
    decide: [{ event: 'PermissionRequest', timeout: 5, scenario: { delayMs: 30000, output: allowPermissionRequest } }],
  },
  's5-ask-pre': {
    prompt: askPrompt,
    decide: [{ event: 'PreToolUse', matcher: 'AskUserQuestion', scenario: { delayMs: 1500, build: 'askAnswerPreToolUse' } }],
  },
  's5-ask-perm': {
    prompt: askPrompt,
    decide: [{ event: 'PermissionRequest', matcher: 'AskUserQuestion', scenario: { delayMs: 1500, build: 'askAnswerPermissionRequest' } }],
  },
  's5-plan': {
    prompt: planPrompt,
    modeArgs: ['--permission-mode', 'plan'],
    decide: [{ event: 'PermissionRequest', matcher: 'ExitPlanMode', scenario: { delayMs: 1500, build: 'planApprovePermissionRequest' } }],
  },
  's5-plan-pre': {
    prompt: planPrompt,
    modeArgs: ['--permission-mode', 'plan'],
    decide: [{ event: 'PreToolUse', matcher: 'ExitPlanMode', scenario: { delayMs: 1500, build: 'planApprovePreToolUse' } }],
  },
  's6a-stop': {
    prompt: 'Reply with the single word READY.',
    decide: [{ event: 'Stop', scenario: { delayMs: 500, build: 'stopContinueOnce' } }],
  },
  's6d-stream': {
    drive: async (session) => {
      session.send('Reply with the single word ONE.');
      await session.waitFor(isResult);
      const agents = await runProcess('claude', ['agents', '--json'], process.env);
      writeFileSync(join(session.runDir, 'agents.json'), JSON.stringify(agents, null, 2));
      session.send('Reply with the single word TWO.');
      await session.waitFor(isResult);
      session.end();
    },
  },
  's6b-rewake': {
    decide: [{
      event: 'Stop',
      asyncRewake: true,
      timeout: 60,
      scenario: { delayMs: 12000, exitCode: 2, firstOnly: true, stderr: 'The user sent this follow-up from Beacon: reply with exactly the single word MANGO.' },
    }],
    drive: async (session) => {
      session.send('Reply with the single word READY.');
      await session.waitFor(isResult);
      if (await session.waitFor(isResult, 60000)) await sleep(5000);
      session.end();
    },
  },
  's6c-resume-live': {
    drive: async (session) => {
      session.send('Reply with the single word ALPHA.');
      const firstResult = await session.waitFor(isResult);
      const transcriptPath = readEvents(session.runDir).find((record) => record.event === 'SessionStart')?.payload.transcript_path;
      const projectFolder = dirname(transcriptPath);
      const before = listTranscripts(projectFolder);
      const transcriptBefore = readFileSync(transcriptPath, 'utf8');
      const resumed = await runProcess(
        'claude',
        ['-p', '--resume', firstResult.session_id, '--model', 'haiku', '--output-format', 'json', 'Reply with the single word BRAVO.'],
        strippedEnv({ BEACON_SPIKE_OUT: session.runDir }),
      );
      const transcriptAfter = readFileSync(transcriptPath, 'utf8');
      const after = listTranscripts(projectFolder);
      writeFileSync(join(session.runDir, 'resume.json'), JSON.stringify({
        sessionA: firstResult.session_id,
        transcriptPath,
        resumed,
        transcriptLinesBefore: transcriptBefore.split('\n').length,
        transcriptLinesAfter: transcriptAfter.split('\n').length,
        transcriptGainedBravo: !transcriptBefore.includes('BRAVO') && transcriptAfter.includes('BRAVO'),
        newTranscripts: after.filter((file) => !before.includes(file)),
      }, null, 2));
      session.send('What was the last word I asked you to reply with? Answer with one word.');
      await session.waitFor(isResult);
      session.end();
    },
  },
  's3-host-race': {
    extraArgs: stdioHost,
    decide: [{ event: 'PermissionRequest', scenario: { delayMs: 20000, output: allowPermissionRequest } }],
    drive: hostAnswersAfterThreeSeconds((request) => ({ behavior: 'allow', updatedInput: request.request.input })),
  },
  's3-hook-first': {
    extraArgs: stdioHost,
    decide: [{ event: 'PermissionRequest', scenario: { delayMs: 2000, output: allowPermissionRequest } }],
    drive: silentHost(bashPrompt),
  },
  's3-host-deny': {
    extraArgs: stdioHost,
    decide: [{ event: 'PermissionRequest', scenario: { delayMs: 20000, output: allowPermissionRequest } }],
    drive: hostAnswersAfterThreeSeconds(() => ({ behavior: 'deny', message: 'Denied by host in spike; just say DENIED.' })),
  },
  's5b-ask-pre': {
    extraArgs: stdioHost,
    decide: [{ event: 'PreToolUse', matcher: 'AskUserQuestion', scenario: { delayMs: 1500, build: 'askAnswerPreToolUse' } }],
    drive: silentHost(askPrompt),
  },
  's5b-ask-perm': {
    extraArgs: stdioHost,
    decide: [{ event: 'PermissionRequest', matcher: 'AskUserQuestion', scenario: { delayMs: 1500, build: 'askAnswerPermissionRequest' } }],
    drive: silentHost(askPrompt),
  },
  's5b-plan': {
    modeArgs: ['--permission-mode', 'plan'],
    extraArgs: stdioHost,
    decide: [{ event: 'PermissionRequest', matcher: 'ExitPlanMode', scenario: { delayMs: 1500, build: 'planApprovePermissionRequest' } }],
    drive: silentHost(silentPlanPrompt),
  },
  's6a-context': {
    prompt: 'Reply with the single word READY.',
    decide: [{ event: 'Stop', scenario: { delayMs: 500, build: 'stopContextOnce' } }],
  },
  's6f-inbox': {
    extraHandlers: [{ event: 'SessionStart', script: 'hook-inbox-capture.mjs' }],
    drive: async (session) => {
      session.send('Reply with the single word READY.');
      await session.waitFor(isResult);
      const { socket, token } = JSON.parse(readFileSync(join(session.runDir, 'inbox.json'), 'utf8'));
      const followUpLines = session.inputs[0] ? JSON.parse(readFileSync(session.inputs[0], 'utf8')) : [];
      await postToInbox(socket, [JSON.stringify({ type: 'auth', token }), ...followUpLines], session.runDir);
      if (await session.waitFor(isResult, 45000)) await sleep(3000);
      session.end();
    },
  },
};

function silentHost(prompt) {
  return async (session) => {
    session.send(prompt);
    await session.waitFor(isResult, 120000);
    await sleep(5000);
    session.end();
  };
}

function hostAnswersAfterThreeSeconds(buildResponse) {
  return async (session) => {
    session.send(bashPrompt);
    const request = await session.waitFor((line) => line.type === 'control_request' || isResult(line));
    if (request?.type === 'control_request') {
      await sleep(3000);
      session.write({ type: 'control_response', response: { subtype: 'success', request_id: request.request_id, response: buildResponse(request) } });
      await session.waitFor(isResult);
      await sleep(25000);
    }
    session.end();
  };
}

function postToInbox(socket, lines, runDir) {
  const record = (entry) => appendFileSync(join(runDir, 'inbox-log.txt'), `${new Date().toISOString()} ${entry}\n`);
  return new Promise((resolvePost) => {
    const pipe = connect(socket);
    pipe.on('data', (chunk) => record(`recv ${JSON.stringify(String(chunk))}`));
    pipe.on('error', (error) => record(`error ${error.message}`));
    pipe.on('close', () => {
      record('closed');
      resolvePost();
    });
    pipe.on('connect', async () => {
      record('connected');
      for (const line of lines) {
        record(`send ${line.includes('"auth"') ? '{"type":"auth","token":"<redacted>"}' : line}`);
        pipe.write(line + '\n');
        await sleep(2000);
      }
      await sleep(5000);
      pipe.end();
    });
  });
}

function strippedEnv(extra) {
  return {
    ...Object.fromEntries(Object.entries(process.env).filter(([name]) => !/^(CLAUDE|ANTHROPIC)/i.test(name))),
    ...extra,
  };
}

function readEvents(runDir) {
  const file = join(runDir, 'events.jsonl');
  return existsSync(file) ? readFileSync(file, 'utf8').split('\n').filter(Boolean).map((line) => JSON.parse(line)) : [];
}

function listTranscripts(folder) {
  return readdirSync(folder).filter((file) => file.endsWith('.jsonl') && statSync(join(folder, file)).isFile());
}

function runProcess(command, args, env) {
  mkdirSync(workDir, { recursive: true });
  return new Promise((resolveRun) => {
    const child = spawn(command, args, { cwd: workDir, env, windowsHide: true });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => (stdout += chunk));
    child.stderr.on('data', (chunk) => (stderr += chunk));
    child.stdin.end();
    child.on('close', (exitCode) => resolveRun({ args, exitCode, stdout, stderr }));
  });
}

function hookHandler(script, ...args) {
  return { type: 'command', command: process.execPath, args: [join(spikesDir, script), ...args] };
}

function buildSettings(name) {
  const hooks = Object.fromEntries(loggerEvents.map((event) => [event, [{ hooks: [hookHandler('hook-logger.mjs', name)] }]]));
  for (const decide of scenarios[name].decide ?? []) {
    hooks[decide.event].push({
      ...(decide.matcher && { matcher: decide.matcher }),
      hooks: [{
        ...hookHandler('hook-decide.mjs', `decide-${decide.event}`, JSON.stringify(decide.scenario)),
        ...(decide.timeout && { timeout: decide.timeout }),
        ...(decide.asyncRewake && { asyncRewake: true }),
      }],
    });
  }
  for (const extra of scenarios[name].extraHandlers ?? []) hooks[extra.event].push({ hooks: [hookHandler(extra.script)] });
  return { hooks };
}

function writeSettings(name) {
  if (!scenarios[name]) throw new Error(`Unknown scenario ${name}`);
  const file = join(outRoot, 'settings', `${name}.json`);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, JSON.stringify(buildSettings(name), null, 2));
  return file;
}

function writeProbe(dir) {
  const file = join(resolve(dir), '.claude', 'settings.json');
  const hook = hookHandler('hook-logger.mjs', 'probe', join(outRoot, 'probe'));
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, JSON.stringify({ hooks: Object.fromEntries(loggerEvents.map((event) => [event, [{ hooks: [hook] }]])) }, null, 2));
  return file;
}

function sanitizeFixtureString(value) {
  const userName = basename(homedir()).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const profilePath = new RegExp(`([a-z]:|/mnt/[a-z]|/[a-z])([\\\\/])Users\\2(?:${userName}|\\w{1,6}~\\d)`, 'gi');
  const sanitized = value.replace(profilePath, (match, drive, separator) => `${drive}${separator}Users${separator}user`);
  return sanitized.length > 400 ? `${sanitized.slice(0, 400)}…` : sanitized;
}

function writeFixtures() {
  const fixturesDir = join(spikesDir, '..', 'fixtures', 'hook-payloads');
  const runsDir = join(outRoot, 'runs');
  const runOrder = ['s2-allow', 's1-observe', ...readdirSync(runsDir)];
  const payloads = new Map();
  for (const record of runOrder.flatMap((run) => readEvents(join(runsDir, run)))) {
    if (record.payload && !record.label.startsWith('decide') && !payloads.has(record.event)) payloads.set(record.event, record.payload);
  }
  mkdirSync(fixturesDir, { recursive: true });
  for (const [event, payload] of payloads) {
    writeFileSync(
      join(fixturesDir, `${event}.json`),
      JSON.stringify(payload, (key, value) => (typeof value === 'string' ? sanitizeFixtureString(value) : value), 2) + '\n',
    );
  }
  return [...payloads.keys()];
}

async function runScenario(name, inputs) {
  const scenario = scenarios[name];
  const settingsFile = writeSettings(name);
  const runDir = join(outRoot, 'runs', name);
  rmSync(runDir, { recursive: true, force: true });
  mkdirSync(runDir, { recursive: true });
  mkdirSync(workDir, { recursive: true });
  writeFileSync(join(workDir, 'notes.txt'), 'Beacon spike notes.\n');
  const args = [
    '-p', '--model', 'haiku',
    ...(scenario.modeArgs ?? ['--permission-mode', 'manual']),
    '--settings', settingsFile,
    '--output-format', 'stream-json', '--verbose', '--include-hook-events',
    ...(scenario.extraArgs ?? []),
    ...(scenario.drive ? ['--input-format', 'stream-json'] : [scenario.prompt]),
  ];
  const startedAt = Date.now();
  const timeline = (entry) => appendFileSync(join(runDir, 'timeline.txt'), `${new Date().toISOString()} +${Date.now() - startedAt}ms ${entry}\n`);
  const child = spawn('claude', args, { cwd: workDir, env: strippedEnv({ BEACON_SPIKE_OUT: runDir }), windowsHide: true });
  let hardKilled = false;
  const killTimer = setTimeout(() => {
    hardKilled = true;
    timeline('hard-kill');
    child.kill();
  }, hardKillMs);
  child.stderr.on('data', (chunk) => appendFileSync(join(runDir, 'stderr.txt'), chunk));

  const lines = [];
  const waiters = [];
  let cursor = 0;
  const settleWaiters = () => {
    for (const waiter of [...waiters]) {
      const index = lines.findIndex((line, position) => position >= cursor && waiter.predicate(line));
      if (index === -1) continue;
      cursor = index + 1;
      waiters.splice(waiters.indexOf(waiter), 1);
      clearTimeout(waiter.timer);
      waiter.resolveWait(lines[index]);
    }
  };
  createInterface({ input: child.stdout }).on('line', (raw) => {
    appendFileSync(join(runDir, 'stream.jsonl'), raw + '\n');
    console.log(raw.length > 400 ? `${raw.slice(0, 400)}…` : raw);
    let parsed;
    try {
      parsed = JSON.parse(raw);
    } catch {
      parsed = { type: 'unparsed' };
    }
    timeline(`stdout ${parsed.type}${parsed.subtype ? `/${parsed.subtype}` : ''}`);
    if (parsed.type?.startsWith('control_')) appendFileSync(join(runDir, 'control_requests.jsonl'), raw + '\n');
    lines.push({ ...parsed, raw });
    settleWaiters();
  });
  const exited = new Promise((resolveExit) => child.on('close', (exitCode, signal) => resolveExit({ exitCode, signal })));
  exited.then(() => {
    for (const waiter of waiters.splice(0)) waiter.resolveWait(null);
  });

  const session = {
    runDir,
    inputs,
    write: (message) => {
      timeline(`stdin ${JSON.stringify(message).slice(0, 200)}`);
      child.stdin.write(JSON.stringify(message) + '\n');
    },
    send: (text) => session.write({ type: 'user', message: { role: 'user', content: text }, parent_tool_use_id: null }),
    end: () => {
      timeline('stdin end');
      child.stdin.end();
    },
    waitFor: (predicate, timeoutMs = hardKillMs) =>
      new Promise((resolveWait) => {
        const waiter = { predicate, resolveWait, timer: setTimeout(() => {
          waiters.splice(waiters.indexOf(waiter), 1);
          resolveWait(null);
        }, timeoutMs) };
        waiters.push(waiter);
        settleWaiters();
      }),
  };

  if (scenario.drive) await scenario.drive(session);
  else child.stdin.end();
  const { exitCode, signal } = await exited;
  clearTimeout(killTimer);
  timeline(`exit ${exitCode} ${signal ?? ''}`);
  writeFileSync(join(runDir, 'meta.json'), JSON.stringify({ args, exitCode, signal, hardKilled, durationMs: Date.now() - startedAt }, null, 2));
  console.log(`run folder: ${runDir} exit ${exitCode}`);
}

const [command, target, ...inputs] = process.argv.slice(2);
if (command === 'list') console.log(Object.keys(scenarios).join('\n'));
else if (command === 'settings') console.log(writeSettings(target));
else if (command === 'run') await runScenario(target, inputs);
else if (command === 'probe') console.log(writeProbe(target));
else if (command === 'auth') console.log(JSON.stringify(await runProcess('claude', ['auth', 'status'], strippedEnv({})), null, 2));
else if (command === 'fixtures') console.log(writeFixtures().join('\n'));
else throw new Error('Usage: node spikes/harness.mjs list | settings <scenario> | run <scenario> [inboxLinesFile] | probe <dir> | auth | fixtures');
