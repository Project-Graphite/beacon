import { useLayoutEffect, useRef, type RefObject } from 'react';
import { listChanges, staggerDelay, type Offset } from '../lib/motion.ts';
import { easing, play } from '../motion.ts';

const ENTER: Keyframe[] = [
  { opacity: 0, transform: 'translateY(-6px)' },
  { opacity: 1, transform: 'none' },
];
const LEAVE: Keyframe[] = [{ opacity: 1 }, { opacity: 0, transform: 'scaleY(0.6)' }];

interface Placement {
  node: HTMLElement;
  offset: Offset;
  width: number;
}

function offsetWithin(node: HTMLElement, container: HTMLElement): Offset {
  const offset = { x: 0, y: 0 };
  for (let current: Element | null = node; current instanceof HTMLElement && current !== container; current = current.offsetParent) {
    offset.x += current.offsetLeft;
    offset.y += current.offsetTop;
  }
  return offset;
}

const place = (container: HTMLElement) =>
  new Map<string, Placement>(
    [...container.querySelectorAll<HTMLElement>('[data-motion]')].map((node) => [
      node.dataset.motion ?? '',
      { node, offset: offsetWithin(node, container), width: node.offsetWidth },
    ]),
  );

const topLevel = (node: HTMLElement) => !node.parentElement?.closest('[data-motion]');

const enter = (nodes: HTMLElement[]) =>
  nodes.forEach((node, index) => {
    const delay = staggerDelay(index);
    if (delay !== null) play(node, ENTER, { duration: 160, delay, easing: easing('settle'), fill: 'backwards' });
  });

export function useListMotion(
  container: RefObject<HTMLElement | null>,
  ghosts: RefObject<HTMLElement | null>,
  key: string | null,
  entrance: number,
) {
  const committedKey = useRef<string | null>(null);
  const before = useRef<Map<string, Placement> | null>(null);
  if (key !== committedKey.current) {
    before.current = container.current && key !== null && committedKey.current !== null ? place(container.current) : null;
  }

  useLayoutEffect(() => {
    committedKey.current = key;
    const [root, layer, previous] = [container.current, ghosts.current, before.current];
    if (!root || !layer || !previous) return;
    const { entered, moved, left } = listChanges(previous, place(root));
    moved
      .filter(({ item }) => topLevel(item.node))
      .forEach(({ item, delta }) =>
        play(item.node, [{ transform: `translate(${delta.x}px, ${delta.y}px)` }, { transform: 'none' }], {
          duration: 220,
          easing: easing('spring'),
        }),
      );
    enter(entered.map((item) => item.node));
    const ghosted: HTMLElement[] = [];
    left.forEach(({ node, offset, width }) => {
      if (node.isConnected || ghosted.some((ghost) => ghost.contains(node))) return;
      ghosted.push(node);
      [node, ...node.querySelectorAll('[data-motion], [id]')].forEach((each) => {
        each.removeAttribute('data-motion');
        each.removeAttribute('id');
      });
      Object.assign(node.style, {
        position: 'absolute',
        left: `${offset.x}px`,
        top: `${offset.y}px`,
        width: `${width}px`,
        transformOrigin: 'top',
      });
      layer.append(node);
      const leaving = play(node, LEAVE, { duration: 160, easing: easing('settle'), fill: 'forwards' });
      if (leaving) leaving.onfinish = () => node.remove();
      else node.remove();
    });
  }, [container, ghosts, key]);

  useLayoutEffect(() => {
    const root = container.current;
    if (entrance > 0 && root) enter([...root.querySelectorAll<HTMLElement>('[data-motion]')].filter(topLevel));
  }, [container, entrance]);
}
