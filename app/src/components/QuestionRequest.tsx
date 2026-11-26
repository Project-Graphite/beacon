import { useEffect, useState } from 'react';
import type { Decision, PendingDecision } from '../beacon';
import { NO_CHOICE, assembleAnswers, pickOption, pickOther, readQuestions, typeOther, type Choice } from '../lib/answers.ts';
import { redact } from '../lib/redact.ts';
import { FilledButton } from './FilledButton';
import { JumpButton } from './JumpButton';
import { KeyHint } from './KeyHint';
import { OutlineButton } from './OutlineButton';

const DISMISSED = 'The user dismissed the question.';
const CHOICE_BOX =
  'grid size-3.5 shrink-0 appearance-none place-items-center border border-faint bg-raised transition-colors duration-120 ease-settle checked:border-ink checked:bg-ink disabled:opacity-45';
const CHOICE = {
  radio: `${CHOICE_BOX} rounded-full checked:shadow-[inset_0_0_0_3px_var(--color-raised)]`,
  checkbox: `${CHOICE_BOX} rounded-mark checked:before:h-2 checked:before:w-1 checked:before:-translate-y-px checked:before:rotate-45 checked:before:border-r-[1.5px] checked:before:border-b-[1.5px] checked:before:border-paper`,
};

interface QuestionRequestProps {
  request: PendingDecision;
  masked: boolean;
  sending: boolean;
  keys: Map<number, (key: string) => void> | null;
  send: (decision: Decision) => void;
  onJump: () => void;
}

export function QuestionRequest({ request, masked, sending, keys, send, onJump }: QuestionRequestProps) {
  const questions = readQuestions(request.tool_input);
  const shown = (text: string) => (masked ? redact(text) : text);
  const [choices, setChoices] = useState<Choice[]>(() => questions.map(() => NO_CHOICE));
  const answers = assembleAnswers(questions, choices);
  const choose = (index: number, choice: Choice) =>
    setChoices((current) => current.map((each, position) => (position === index ? choice : each)));
  const submit = () => answers && send({ type: 'answer', answers });
  const dismiss = () => send({ type: 'deny', message: DISMISSED });

  useEffect(() => {
    if (!keys) return;
    keys.set(request.id, (key) => {
      if (sending) return;
      if (key === 'a') submit();
      if (key === 'd') dismiss();
    });
    return () => void keys.delete(request.id);
  });

  return (
    <fieldset disabled={sending} className="min-w-0">
      {questions.map((question, index) => {
        const choice = choices[index] ?? NO_CHOICE;
        const group = `question-${request.id}-${index}`;
        const kind = question.multiSelect ? 'checkbox' : 'radio';
        return (
          <div key={group} className="mt-2 first:mt-1">
            {question.header && (
              <span className="inline-block rounded-mark border border-line px-1.5 text-micro text-muted">
                {shown(question.header)}
              </span>
            )}
            <p className="mt-1 text-secondary text-ink">{shown(question.question)}</p>
            <div className="mt-1 space-y-0.5">
              {question.options.map((option) => (
                <label
                  key={option.label}
                  className="flex cursor-pointer items-start gap-2 rounded-field px-1.5 py-1 transition-colors duration-120 ease-settle hover:bg-surface"
                >
                  <input
                    type={kind}
                    name={group}
                    checked={choice.labels.includes(option.label)}
                    onChange={() => choose(index, pickOption(question, choice, option.label))}
                    className={`mt-0.5 ${CHOICE[kind]}`}
                  />
                  <span className="min-w-0">
                    <span className="text-secondary text-ink">{shown(option.label)}</span>
                    {option.description && (
                      <span className="block text-micro text-muted">{shown(option.description)}</span>
                    )}
                  </span>
                </label>
              ))}
              <label className="flex items-center gap-2 rounded-field px-1.5 py-1">
                <input
                  type={kind}
                  name={group}
                  checked={choice.other !== null}
                  onChange={() => choose(index, pickOther(question, choice))}
                  className={CHOICE[kind]}
                />
                <span className="text-secondary text-ink">Other</span>
                <input
                  aria-label={`Other answer to ${shown(question.question)}`}
                  value={choice.other ?? ''}
                  placeholder="Type an answer"
                  onChange={(event) => choose(index, typeOther(question, choice, event.target.value))}
                  className="h-7 min-w-0 flex-1 rounded-field border border-line bg-raised px-2.5 text-secondary text-ink shadow-card placeholder:text-faint"
                />
              </label>
            </div>
          </div>
        );
      })}
      <div className="mt-2 flex flex-wrap items-center gap-1.5">
        <FilledButton disabled={sending || answers === null} onClick={submit}>
          Send answer
          <KeyHint shortcut="A" />
        </FilledButton>
        <OutlineButton disabled={sending} onClick={dismiss}>
          Dismiss
          <KeyHint shortcut="D" />
        </OutlineButton>
        <JumpButton shortcut onJump={onJump} />
      </div>
    </fieldset>
  );
}
