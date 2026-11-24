import type { ButtonHTMLAttributes } from 'react';

export function OutlineButton({ compact, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { compact?: boolean }) {
  return (
    <button
      type="button"
      className={`press inline-flex shrink-0 items-center justify-center rounded-field border border-line bg-raised text-secondary font-medium text-ink shadow-card hover:border-faint/60 hover:bg-surface disabled:pointer-events-none disabled:opacity-45 ${compact ? 'h-[26px] px-2.5' : 'h-7 px-3'}`}
      {...props}
    />
  );
}
