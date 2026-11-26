import type { ButtonHTMLAttributes } from 'react';

export function TextButton(props: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      className="press -mx-1 inline-flex min-h-6 items-center shrink-0 rounded-field px-1 py-0.5 text-secondary text-muted hover:text-ink disabled:pointer-events-none disabled:opacity-45"
      {...props}
    />
  );
}
