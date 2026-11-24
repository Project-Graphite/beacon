export function EmptyState({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="flex animate-appear flex-col items-center justify-center gap-1 px-6 py-10 text-center">
      <p className="font-medium text-ink">{title}</p>
      <p className="text-secondary text-muted">{detail}</p>
    </div>
  );
}
