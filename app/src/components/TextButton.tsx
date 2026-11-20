import type { ButtonHTMLAttributes } from 'react';

export function TextButton(props: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      className="press shrink-0 rounded-field text-micro text-muted hover:text-ink disabled:pointer-events-none disabled:opacity-50"
      {...props}
    />
  );
}
