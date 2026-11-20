import { useEffect, useState } from 'react';
import type { Decision, PendingDecision } from '../beacon';
import { NO_CHOICE, assembleAnswers, pickOption, pickOther, readQuestions, typeOther, type Choice } from '../lib/answers.ts';
import { FilledButton } from './FilledButton';
import { KeyHint } from './KeyHint';
import { OutlineButton } from './OutlineButton';

const DISMISSED = 'The user dismissed the question.';

interface QuestionRequestProps {
  request: PendingDecision;
  sending: boolean;
  keys: Map<number, (key: string) => void>;
  send: (decision: Decision) => void;
}

export function QuestionRequest({ request, sending, keys, send }: QuestionRequestProps) {
  const questions = readQuestions(request.tool_input);
  const [choices, setChoices] = useState<Choice[]>(() => questions.map(() => NO_CHOICE));
  const answers = assembleAnswers(questions, choices);
  const choose = (index: number, choice: Choice) =>
    setChoices((current) => current.map((each, position) => (position === index ? choice : each)));
  const submit = () => answers && send({ type: 'answer', answers });
  const dismiss = () => send({ type: 'deny', message: DISMISSED });

  useEffect(() => {
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
                {question.header}
              </span>
            )}
            <p className="mt-1 text-secondary text-ink">{question.question}</p>
            <div className="mt-1 space-y-0.5">
              {question.options.map((option) => (
                <label
                  key={option.label}
                  className="flex cursor-pointer items-baseline gap-2 rounded-field px-1.5 py-1 transition-colors duration-150 ease-settle hover:bg-surface"
                >
                  <input
                    type={kind}
                    name={group}
                    checked={choice.labels.includes(option.label)}
                    onChange={() => choose(index, pickOption(question, choice, option.label))}
                    className="translate-y-px accent-ink"
                  />
                  <span className="min-w-0">
                    <span className="text-secondary text-ink">{option.label}</span>
                    {option.description && (
                      <span className="block text-micro text-muted">{option.description}</span>
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
                  className="accent-ink"
                />
                <span className="text-secondary text-ink">Other</span>
                <input
                  aria-label={`Other answer to ${question.question}`}
                  value={choice.other ?? ''}
                  placeholder="Type an answer"
                  onChange={(event) => choose(index, typeOther(question, choice, event.target.value))}
                  className="h-6 min-w-0 flex-1 rounded-field border border-line bg-paper px-2 text-secondary text-ink placeholder:text-faint focus:border-muted focus:outline-none"
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
      </div>
    </fieldset>
  );
}
