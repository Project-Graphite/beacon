import { snapshot, ui } from './fixture';

type Handler = (event: { payload: unknown }) => void;

const handlers = new Map<string, Set<Handler>>();

export function emit(name: string, payload: unknown) {
  handlers.get(name)?.forEach((handler) => handler({ payload }));
}

export async function listen(name: string, handler: Handler) {
  const set = handlers.get(name) ?? new Set<Handler>();
  set.add(handler);
  handlers.set(name, set);
  if (name === 'beacon://opened') {
    window.setTimeout(() => emit(name, { mode: 'sticky', focused: true, edge: 'right', snapshot, ui }), 50);
  }
  return () => set.delete(handler);
}
