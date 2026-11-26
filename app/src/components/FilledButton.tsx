import type { ButtonHTMLAttributes } from 'react';

export function FilledButton({ compact, danger, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { compact?: boolean; danger?: boolean }) {
  return (
    <button
      type="button"
      className={`press inline-flex shrink-0 items-center justify-center rounded-field border border-transparent text-secondary font-medium text-paper shadow-card disabled:pointer-events-none disabled:border-line disabled:bg-raised disabled:text-ink disabled:opacity-45 [&_kbd]:text-current [&_kbd]:opacity-60 ${danger ? 'bg-danger hover:bg-danger/88' : 'bg-ink hover:bg-ink/88'} ${compact ? 'h-[26px] px-2.5' : 'h-7 px-3'}`}
      {...props}
    />
  );
}
