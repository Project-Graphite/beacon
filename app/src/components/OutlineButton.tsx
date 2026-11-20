import type { ButtonHTMLAttributes } from 'react';

export function OutlineButton({ compact, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { compact?: boolean }) {
  return (
    <button
      type="button"
      className={`press inline-flex shrink-0 items-center rounded-field border border-line text-secondary text-ink hover:border-muted hover:bg-line-soft disabled:pointer-events-none disabled:opacity-50 ${compact ? 'h-[22px] px-2' : 'h-6 px-2.5'}`}
      {...props}
    />
  );
}
