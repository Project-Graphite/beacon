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
      className={`relative h-5 w-9 shrink-0 rounded-full border transition-colors duration-200 ease-settle disabled:cursor-default ${checked ? 'border-ink bg-ink' : 'border-line bg-line hover:border-muted'}`}
    >
      <span
        aria-hidden="true"
        className={`absolute top-px left-px size-4 rounded-full shadow-card transition-[translate,background-color] duration-200 ease-spring ${checked ? 'translate-x-4 bg-paper' : 'bg-muted'}`}
      />
    </button>
  );
}
