import type { ButtonHTMLAttributes } from 'react';

export function FilledButton({ compact, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { compact?: boolean }) {
  return (
    <button
      type="button"
      className={`press inline-flex shrink-0 items-center justify-center rounded-field bg-ink text-secondary font-medium text-paper shadow-card hover:bg-ink/88 disabled:pointer-events-none disabled:opacity-45 ${compact ? 'h-[26px] px-2.5' : 'h-7 px-3'}`}
      {...props}
    />
  );
}
