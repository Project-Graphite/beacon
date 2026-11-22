import { useSyncExternalStore } from 'react';

const TICK_MS = 1000;
const listeners = new Set<() => void>();
let current = Date.now();
let timer = 0;

const tick = () => {
  current = Date.now();
  listeners.forEach((listener) => listener());
};

const run = () => {
  window.clearInterval(timer);
  timer = listeners.size > 0 && !document.hidden ? window.setInterval(tick, TICK_MS) : 0;
  if (timer) tick();
};

document.addEventListener('visibilitychange', run);

const subscribe = (listener: () => void) => {
  current = Date.now();
  listeners.add(listener);
  if (listeners.size === 1) run();
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) run();
  };
};

const still = () => () => {};
const read = () => current;

export function useNow(ticking: boolean): number {
  return useSyncExternalStore(ticking ? subscribe : still, read);
}
