import type { ButtonHTMLAttributes } from 'react';

export function OutlineButton(props: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      className="h-6 shrink-0 rounded-field border border-line px-2.5 text-secondary text-ink transition-colors duration-150 ease-settle hover:border-muted hover:bg-line-soft"
      {...props}
    />
  );
}
