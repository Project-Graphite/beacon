import type { ButtonHTMLAttributes } from 'react';

export function FilledButton({ compact, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { compact?: boolean }) {
  return (
    <button
      type="button"
      className={`press inline-flex shrink-0 items-center rounded-field border border-ink bg-ink text-secondary font-medium text-paper hover:bg-ink/85 disabled:pointer-events-none disabled:opacity-50 ${compact ? 'h-[22px] px-2' : 'h-6 px-2.5'}`}
      {...props}
    />
  );
}
