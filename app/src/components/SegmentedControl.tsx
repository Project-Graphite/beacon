import type { Choice } from '../lib/settings.ts';
import type { Field } from './SettingsRow';

interface SegmentedControlProps<T extends string | number> {
  field: Field;
  choices: Choice<T>[];
  value: T;
  disabled?: boolean;
  onChange: (value: T) => void;
}

export function SegmentedControl<T extends string | number>({
  field,
  choices,
  value,
  disabled,
  onChange,
}: SegmentedControlProps<T>) {
  return (
    <div
      role="radiogroup"
      id={field.id}
      aria-labelledby={field.labelId}
      aria-describedby={field.helpId}
      className="flex h-7 shrink-0 rounded-field bg-surface p-[3px]"
    >
      {choices.map((choice) => (
        <label
          key={choice.value}
          className={`relative flex items-center rounded-mark px-2.5 text-secondary text-muted transition-[color,background-color,box-shadow] duration-200 ease-settle has-checked:bg-segment has-checked:text-ink has-checked:shadow-card has-focus-visible:outline-2 has-focus-visible:outline-ink ${disabled ? '' : 'cursor-pointer hover:text-ink'}`}
        >
          <input
            type="radio"
            name={field.id}
            value={choice.value}
            checked={choice.value === value}
            disabled={disabled}
            onChange={() => onChange(choice.value)}
            className="absolute inset-0 m-0 cursor-[inherit] appearance-none rounded-mark outline-none"
          />
          <span className="whitespace-nowrap">{choice.label}</span>
        </label>
      ))}
    </div>
  );
}
