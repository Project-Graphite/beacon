import type { ButtonHTMLAttributes } from 'react';

export function TextButton(props: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      className="press shrink-0 rounded-[6px] px-1 -mx-1 text-secondary text-muted hover:text-ink disabled:pointer-events-none disabled:opacity-45"
      {...props}
    />
  );
}
