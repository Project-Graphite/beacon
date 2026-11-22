import { useId, type ReactNode } from 'react';

export interface Field {
  id: string;
  labelId: string;
  helpId: string | undefined;
}

interface SettingsRowProps {
  label: string;
  help?: string;
  disabled?: boolean;
  children: (field: Field) => ReactNode;
}

export function SettingsRow({ label, help, disabled, children }: SettingsRowProps) {
  const id = useId();
  const field = { id, labelId: `${id}label`, helpId: help && `${id}help` };
  return (
    <div title={help} className={`flex h-7 items-center justify-between gap-3 px-2.5 ${disabled ? 'opacity-50' : ''}`}>
      <label id={field.labelId} htmlFor={id} className="shrink-0 text-secondary text-ink">
        {label}
      </label>
      {help && (
        <span id={field.helpId} hidden>
          {help}
        </span>
      )}
      {children(field)}
    </div>
  );
}
