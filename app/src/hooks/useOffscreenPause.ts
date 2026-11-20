import { useEffect, useRef } from 'react';

let observer: IntersectionObserver | null = null;

const watcher = () =>
  (observer ??= new IntersectionObserver((entries) =>
    entries.forEach((entry) => entry.target.toggleAttribute('data-offscreen', !entry.isIntersecting)),
  ));

export function useOffscreenPause<T extends Element>() {
  const node = useRef<T>(null);
  useEffect(() => {
    const watched = node.current;
    if (!watched) return;
    watcher().observe(watched);
    return () => watcher().unobserve(watched);
  }, []);
  return node;
}
