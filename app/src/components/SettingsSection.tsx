import { useId, type ReactNode } from 'react';
import { Collapse } from './Collapse';
import { Icon } from './Icon';

interface SettingsSectionProps {
  label: string;
  summary: string;
  open: boolean;
  onToggle: () => void;
  error?: string | null;
  children: ReactNode;
}

export function SettingsSection({ label, summary, open, onToggle, error, children }: SettingsSectionProps) {
  const body = useId();
  return (
    <section aria-label={label} className="border-b border-line-soft last:border-b-0">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={body}
        onClick={onToggle}
        className="flex h-11 w-full items-center gap-2 px-4 text-left transition-colors duration-150 ease-settle hover:bg-surface"
      >
        <Icon
          name="chevron"
          className={`size-3.5 shrink-0 text-faint transition-transform duration-200 ease-settle ${open ? 'rotate-90' : ''}`}
        />
        <span className="shrink-0 font-medium text-ink">{label}</span>
        <span
          aria-hidden={open}
          className={`ml-auto min-w-0 truncate text-secondary text-faint transition-opacity duration-200 ease-settle ${open ? 'opacity-0' : ''}`}
        >
          {summary}
        </span>
      </button>
      <div id={body}>
        <Collapse open={open}>
          <div className="pb-2.5">{children}</div>
        </Collapse>
      </div>
      {error && (
        <p role="alert" title={error} className="truncate px-4 pb-2.5 text-secondary text-danger">
          {error}
        </p>
      )}
    </section>
  );
}
