export interface QuestionOption {
  label: string;
  description: string;
}

export interface Question {
  question: string;
  header: string;
  options: QuestionOption[];
  multiSelect: boolean;
}

export interface Choice {
  labels: string[];
  other: string | null;
}

export const NO_CHOICE: Choice = { labels: [], other: null };

const text = (value: unknown) => (typeof value === 'string' ? value : '');

export function readQuestions(toolInput: unknown): Question[] {
  const questions = (toolInput as { questions?: unknown } | null)?.questions;
  return (Array.isArray(questions) ? questions : [])
    .filter((entry) => typeof entry?.question === 'string')
    .map((entry) => ({
      question: entry.question,
      header: text(entry.header),
      options: (Array.isArray(entry.options) ? entry.options : [])
        .filter((option: { label?: unknown }) => typeof option?.label === 'string')
        .map((option: { label: string; description?: unknown }) => ({ label: option.label, description: text(option.description) })),
      multiSelect: entry.multiSelect === true,
    }));
}

export function pickOption(question: Question, choice: Choice, label: string): Choice {
  if (!question.multiSelect) return { labels: [label], other: null };
  const labels = choice.labels.includes(label)
    ? choice.labels.filter((picked) => picked !== label)
    : question.options.map((option) => option.label).filter((each) => each === label || choice.labels.includes(each));
  return { ...choice, labels };
}

export function pickOther(question: Question, choice: Choice): Choice {
  if (!question.multiSelect) return { labels: [], other: choice.other ?? '' };
  return { ...choice, other: choice.other === null ? '' : null };
}

export function typeOther(question: Question, choice: Choice, other: string): Choice {
  return question.multiSelect ? { ...choice, other } : { labels: [], other };
}

export function answerFor(choice: Choice): string | null {
  const parts = [...choice.labels, ...(choice.other?.trim() ? [choice.other.trim()] : [])];
  return parts.length > 0 ? parts.join(', ') : null;
}

export function assembleAnswers(questions: Question[], choices: Choice[]): Record<string, string> | null {
  const answers: Record<string, string> = {};
  for (const [index, question] of questions.entries()) {
    const answer = answerFor(choices[index] ?? NO_CHOICE);
    if (answer === null) return null;
    answers[question.question] = answer;
  }
  return questions.length > 0 ? answers : null;
}
