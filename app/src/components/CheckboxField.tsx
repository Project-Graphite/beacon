import { useId } from 'react';

interface CheckboxFieldProps {
  label: string;
  help?: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}

export function CheckboxField({ label, help, checked, disabled, onChange }: CheckboxFieldProps) {
  const helpId = useId();
  return (
    <label className={`flex items-start gap-2 py-1 ${disabled ? 'opacity-50' : 'cursor-pointer'}`}>
      <span className="relative mt-[3px] grid size-3.5 shrink-0 place-items-center">
        <input
          type="checkbox"
          checked={checked}
          disabled={disabled}
          aria-describedby={help ? helpId : undefined}
          onChange={(event) => onChange(event.target.checked)}
          className="peer absolute inset-0 m-0 cursor-[inherit] appearance-none rounded-[4px] border border-line bg-paper transition-colors duration-150 ease-settle checked:border-ink checked:bg-ink hover:border-muted"
        />
        <svg
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.25"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
          className="pointer-events-none relative size-3 text-paper opacity-0 peer-checked:opacity-100"
        >
          <path d="m3.5 8.5 3 3 6-7" />
        </svg>
      </span>
      <span className="min-w-0">
        <span className="block text-ink">{label}</span>
        {help && (
          <span id={helpId} className="block text-secondary text-muted">
            {help}
          </span>
        )}
      </span>
    </label>
  );
}
