import { useState } from 'react';
import type { Field } from './SettingsRow';

interface SliderProps {
  field: Field;
  value: number;
  range: { min: number; max: number; step: number };
  format: (value: number) => string;
  disabled?: boolean;
  onCommit: (value: number) => Promise<unknown>;
}

export function Slider({ field, value, range, format, disabled, onCommit }: SliderProps) {
  const [draft, setDraft] = useState<number | null>(null);
  const shown = draft ?? value;
  const commit = () => {
    if (draft === null) return;
    if (draft === value) setDraft(null);
    else void onCommit(draft).finally(() => setDraft(null));
  };
  return (
    <span className="flex shrink-0 items-center gap-2">
      <input
        id={field.id}
        type="range"
        min={range.min}
        max={range.max}
        step={range.step}
        value={shown}
        disabled={disabled}
        aria-describedby={field.helpId}
        aria-valuetext={format(shown)}
        onChange={(event) => setDraft(Number(event.target.value))}
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
        className="block h-4 w-24 cursor-pointer appearance-none bg-transparent disabled:cursor-default [&::-webkit-slider-runnable-track]:h-1 [&::-webkit-slider-runnable-track]:rounded-full [&::-webkit-slider-runnable-track]:bg-line [&::-webkit-slider-thumb]:-mt-[5px] [&::-webkit-slider-thumb]:size-3.5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:border-2 [&::-webkit-slider-thumb]:border-paper [&::-webkit-slider-thumb]:bg-ink"
      />
      <output htmlFor={field.id} className="w-8 text-right font-mono text-secondary text-muted tabular-nums">
        {format(shown)}
      </output>
    </span>
  );
}
