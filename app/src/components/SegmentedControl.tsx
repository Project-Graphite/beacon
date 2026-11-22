import { useId } from 'react';
import type { Choice } from '../lib/settings.ts';

interface SegmentedControlProps<T extends string | number> {
  label: string;
  choices: Choice<T>[];
  value: T;
  disabled?: boolean;
  onChange: (value: T) => void;
}

export function SegmentedControl<T extends string | number>({
  label,
  choices,
  value,
  disabled,
  onChange,
}: SegmentedControlProps<T>) {
  const name = useId();
  return (
    <div className={`py-1 ${disabled ? 'opacity-50' : ''}`}>
      <p id={name} className="text-ink">
        {label}
      </p>
      <div
        role="radiogroup"
        aria-labelledby={name}
        className="mt-1 flex rounded-field border border-line bg-paper p-0.5"
      >
        {choices.map((choice) => (
          <label
            key={choice.value}
            className={`relative flex h-[22px] min-w-0 flex-1 items-center justify-center rounded-[6px] text-secondary text-muted transition-colors duration-120 ease-settle has-checked:bg-line has-checked:text-ink has-focus-visible:outline-2 has-focus-visible:outline-ink ${disabled ? '' : 'cursor-pointer hover:text-ink'}`}
          >
            <input
              type="radio"
              name={name}
              value={choice.value}
              checked={choice.value === value}
              disabled={disabled}
              onChange={() => onChange(choice.value)}
              className="absolute inset-0 m-0 cursor-[inherit] appearance-none rounded-[6px] outline-none"
            />
            <span className="truncate">{choice.label}</span>
          </label>
        ))}
      </div>
    </div>
  );
}
