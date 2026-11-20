export function ArgumentList({ entries }: { entries: [string, string][] }) {
  if (entries.length === 0) return <p className="mt-1 text-micro text-muted">No arguments</p>;
  return (
    <dl className="mt-1 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-0.5 rounded-field bg-surface px-2 py-1.5 font-mono text-micro">
      {entries.map(([key, value]) => (
        <div key={key} className="contents">
          <dt className="text-muted">{key}</dt>
          <dd className="text-ink [overflow-wrap:anywhere]">{value}</dd>
        </div>
      ))}
    </dl>
  );
}
