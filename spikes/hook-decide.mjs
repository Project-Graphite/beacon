import { existsSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { appendEvent, readStdin, resolveOutDir } from './spike-log.mjs';

const [label, scenarioJson, explicitOutDir] = process.argv.slice(2);
const outDir = resolveOutDir(explicitOutDir);
const log = (event, extra) => appendEvent(outDir, { label, event, ...extra });

function firstOptionAnswers(questions) {
  return Object.fromEntries(questions.map((question) => [question.question, question.options[0].label]));
}

const builders = {
  askAnswerPreToolUse: (input) => ({
    hookSpecificOutput: {
      hookEventName: 'PreToolUse',
      permissionDecision: 'allow',
      updatedInput: { questions: input.tool_input.questions, answers: firstOptionAnswers(input.tool_input.questions) },
    },
  }),
  askAnswerPermissionRequest: (input) => ({
    hookSpecificOutput: {
      hookEventName: 'PermissionRequest',
      decision: {
        behavior: 'allow',
        updatedInput: { questions: input.tool_input.questions, answers: firstOptionAnswers(input.tool_input.questions) },
      },
    },
  }),
  planApprovePermissionRequest: (input) => ({
    hookSpecificOutput: { hookEventName: 'PermissionRequest', decision: { behavior: 'allow', updatedInput: input.tool_input } },
  }),
  planApprovePreToolUse: (input) => ({
    hookSpecificOutput: { hookEventName: 'PreToolUse', permissionDecision: 'allow', updatedInput: input.tool_input },
  }),
  stopContinueOnce: (input) =>
    input.stop_hook_active
      ? undefined
      : {
          decision: 'block',
          reason: 'The user sent this follow-up from Beacon: reply with exactly the single word PINEAPPLE and nothing else.',
        },
  stopContextOnce: (input) =>
    input.stop_hook_active
      ? undefined
      : {
          hookSpecificOutput: {
            hookEventName: 'Stop',
            additionalContext: 'The user sent this follow-up from Beacon: reply with exactly the single word PAPAYA and nothing else.',
          },
        },
};

process.stdin.on('end', () => log('stdin_end'));
process.stdin.on('close', () => log('stdin_close'));
for (const signal of ['SIGTERM', 'SIGINT', 'SIGBREAK']) {
  process.on(signal, () => {
    log('signal', { signal });
    process.exit(143);
  });
}
process.stdout.on('error', (error) => log('stdout_error', { error: String(error) }));
process.on('exit', (code) => log('exit', { code }));

try {
  const scenario = JSON.parse(scenarioJson);
  const input = JSON.parse(await readStdin());
  log('decide_start', { hookEvent: input.hook_event_name, scenario, payload: input });
  const marker = join(outDir, `once-${label}.marker`);
  if (scenario.firstOnly && existsSync(marker)) {
    log('decide_skip', { hookEvent: input.hook_event_name });
    process.exit(0);
  }
  if (scenario.firstOnly) writeFileSync(marker, new Date().toISOString());
  const heartbeatFile = join(outDir, `heartbeat-${process.pid}.txt`);
  const heartbeat = setInterval(() => writeFileSync(heartbeatFile, new Date().toISOString()), 250);
  writeFileSync(heartbeatFile, new Date().toISOString());
  await new Promise((resolveDelay) => setTimeout(resolveDelay, scenario.delayMs ?? 0));
  clearInterval(heartbeat);
  const output = scenario.build ? builders[scenario.build](input) : scenario.output;
  log('decide_emit', { hookEvent: input.hook_event_name, output, exitCode: scenario.exitCode ?? 0 });
  if (scenario.stderr) process.stderr.write(scenario.stderr);
  if (output !== undefined) process.stdout.write(JSON.stringify(output));
  process.exitCode = scenario.exitCode ?? 0;
} catch (error) {
  log('decide_error', { error: String(error?.stack ?? error) });
  process.exitCode = 0;
}
