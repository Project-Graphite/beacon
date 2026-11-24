interface ProgressBarProps {
  fraction: number;
  alert: boolean;
  className: string;
}

export function ProgressBar({ fraction, alert, className }: ProgressBarProps) {
  return (
    <span aria-hidden="true" className={`block overflow-hidden bg-line-soft ${className}`}>
      <span
        style={{ transform: `scaleX(${fraction})` }}
        className={`block h-full origin-left transition-transform duration-500 ease-settle ${alert ? 'bg-wip' : 'bg-ink/60'}`}
      />
    </span>
  );
}
