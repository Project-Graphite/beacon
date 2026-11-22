import { useId, useState } from 'react';

interface SliderProps {
  label: string;
  value: number;
  range: { min: number; max: number; step: number };
  format: (value: number) => string;
  disabled?: boolean;
  onCommit: (value: number) => Promise<unknown>;
}

export function Slider({ label, value, range, format, disabled, onCommit }: SliderProps) {
  const name = useId();
  const [draft, setDraft] = useState<number | null>(null);
  const shown = draft ?? value;
  const commit = () => {
    if (draft === null) return;
    if (draft === value) setDraft(null);
    else void onCommit(draft).finally(() => setDraft(null));
  };
  return (
    <div className={`py-1 ${disabled ? 'opacity-50' : ''}`}>
      <p className="flex items-baseline justify-between gap-2">
        <label htmlFor={name} className="text-ink">
          {label}
        </label>
        <output htmlFor={name} className="font-mono text-secondary text-muted tabular-nums">
          {format(shown)}
        </output>
      </p>
      <input
        id={name}
        type="range"
        min={range.min}
        max={range.max}
        step={range.step}
        value={shown}
        disabled={disabled}
        aria-valuetext={format(shown)}
        onChange={(event) => setDraft(Number(event.target.value))}
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
        className="mt-1 block h-4 w-full cursor-pointer appearance-none bg-transparent disabled:cursor-default [&::-webkit-slider-runnable-track]:h-1 [&::-webkit-slider-runnable-track]:rounded-full [&::-webkit-slider-runnable-track]:bg-line [&::-webkit-slider-thumb]:-mt-[5px] [&::-webkit-slider-thumb]:size-3.5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:border-2 [&::-webkit-slider-thumb]:border-paper [&::-webkit-slider-thumb]:bg-ink"
      />
    </div>
  );
}
