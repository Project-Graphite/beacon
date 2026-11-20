import type { ReactNode } from 'react';

interface SettingsSectionProps {
  label: string;
  error?: string | null;
  children: ReactNode;
}

export function SettingsSection({ label, error, children }: SettingsSectionProps) {
  return (
    <section aria-label={label} className="border-b border-line-soft p-2.5 last:border-b-0">
      <h2 className="mb-1.5 text-secondary text-faint">{label}</h2>
      {children}
      {error && (
        <p role="alert" title={error} className="mt-1.5 truncate text-micro text-danger">
          {error}
        </p>
      )}
    </section>
  );
}
