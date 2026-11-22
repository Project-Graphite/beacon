import { useId } from 'react';
import type { Choice } from '../lib/settings.ts';

interface RadioListProps<T extends string> {
  label: string;
  choices: Choice<T>[];
  value: T;
  disabled?: boolean;
  onChange: (value: T) => void;
}

export function RadioList<T extends string>({ label, choices, value, disabled, onChange }: RadioListProps<T>) {
  const name = useId();
  return (
    <div className="py-1">
      <p id={name} className="text-ink">
        {label}
      </p>
      <div role="radiogroup" aria-labelledby={name} className="mt-0.5">
        {choices.map((choice) => (
          <label
            key={choice.value}
            className={`flex items-start gap-2 py-0.5 ${disabled ? 'opacity-50' : 'cursor-pointer'}`}
          >
            <span className="relative mt-[3px] grid size-3.5 shrink-0 place-items-center">
              <input
                type="radio"
                name={name}
                value={choice.value}
                checked={choice.value === value}
                disabled={disabled}
                aria-describedby={choice.help ? `${name}${choice.value}` : undefined}
                onChange={() => onChange(choice.value)}
                className="peer absolute inset-0 m-0 cursor-[inherit] appearance-none rounded-full border border-line bg-paper transition-colors duration-120 ease-settle checked:border-ink checked:bg-ink hover:border-muted"
              />
              <span
                aria-hidden="true"
                className="pointer-events-none relative size-1.5 rounded-full bg-paper opacity-0 peer-checked:opacity-100"
              />
            </span>
            <span className="min-w-0">
              <span className="block text-secondary text-ink">{choice.label}</span>
              {choice.help && (
                <span id={`${name}${choice.value}`} className="block truncate text-micro text-muted">
                  {choice.help}
                </span>
              )}
            </span>
          </label>
        ))}
      </div>
    </div>
  );
}
