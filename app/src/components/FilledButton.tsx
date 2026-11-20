import type { ButtonHTMLAttributes } from 'react';

export function FilledButton(props: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      className="inline-flex h-6 shrink-0 items-center rounded-field border border-ink bg-ink px-2.5 text-secondary font-medium text-paper transition-colors duration-150 ease-settle hover:bg-ink/85 disabled:pointer-events-none disabled:opacity-50"
      {...props}
    />
  );
}
