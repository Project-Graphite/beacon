const RUNNING = 'Running ';

export function StepLabel({ label }: { label: string }) {
  return label.startsWith(RUNNING) ? (
    <>
      {RUNNING}
      <span className="font-mono">{label.slice(RUNNING.length)}</span>
    </>
  ) : (
    label
  );
}
