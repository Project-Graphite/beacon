import { cardId } from './lib/sessions.ts';

const FLASH: Keyframe[] = [{ opacity: 0 }, { opacity: 1 }, { opacity: 0 }];

export const easing = (name: 'settle' | 'spring') =>
  getComputedStyle(document.documentElement).getPropertyValue(`--ease-${name}`).trim();

export const play = (node: Element, keyframes: Keyframe[], options: KeyframeAnimationOptions) =>
  matchMedia('(prefers-reduced-motion: reduce)').matches ? null : node.animate(keyframes, options);

export function flashRow(sessionId: string) {
  const flash = document.getElementById(cardId(sessionId))?.querySelector('[data-flash]');
  if (flash) play(flash, FLASH, { duration: 240, easing: easing('settle') });
}
