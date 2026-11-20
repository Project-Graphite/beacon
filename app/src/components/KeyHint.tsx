export function KeyHint({ shortcut }: { shortcut: string }) {
  return (
    <kbd aria-hidden="true" className="ml-1.5 font-mono text-micro font-normal text-faint">
      {shortcut}
    </kbd>
  );
}
