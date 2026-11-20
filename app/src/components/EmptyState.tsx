export function EmptyState({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="flex h-16 flex-col items-center justify-center px-2.5 text-center">
      <p className="text-ink">{title}</p>
      <p className="text-secondary text-muted">{detail}</p>
    </div>
  );
}
