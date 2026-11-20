import type { ButtonHTMLAttributes } from 'react';

export function OutlineButton(props: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      className="inline-flex h-6 shrink-0 items-center rounded-field border border-line px-2.5 text-secondary text-ink transition-colors duration-150 ease-settle hover:border-muted hover:bg-line-soft disabled:pointer-events-none disabled:opacity-50"
      {...props}
    />
  );
}
