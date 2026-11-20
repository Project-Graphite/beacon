export function EmptyState({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="flex h-[120px] flex-col items-center justify-center gap-1 px-3 text-center">
      <p className="text-ink">{title}</p>
      <p className="text-secondary text-muted">{detail}</p>
    </div>
  );
}
