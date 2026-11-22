import type { Field } from './SettingsRow';

interface SwitchProps {
  field: Field;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}

export function Switch({ field, checked, disabled, onChange }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      id={field.id}
      aria-checked={checked}
      aria-describedby={field.helpId}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-4 w-7 shrink-0 rounded-full border disabled:cursor-default ${checked ? 'border-ink bg-ink' : 'border-line bg-line hover:border-muted'}`}
    >
      <span
        aria-hidden="true"
        className={`absolute top-px left-px size-3 rounded-full transition-transform duration-150 ease-settle ${checked ? 'translate-x-3 bg-paper' : 'bg-muted'}`}
      />
    </button>
  );
}
