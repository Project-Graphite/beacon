import { createContext, useCallback, useEffect, useRef, type RefObject } from 'react';
import { setPanelHeight } from '../beacon';

const SCREEN_ALLOWANCE_PX = 24;
const FIT_TIMEOUT_MS = 150;

export const PanelFit = createContext<(extra?: number) => Promise<void>>(() => Promise.resolve());

const shrinking = (content: HTMLElement) =>
  content
    .getAnimations({ subtree: true })
    .filter((animation) => animation.playState === 'running' && animation.effect?.getTiming().iterations !== Infinity);

export function usePanelHeight(
  panel: RefObject<HTMLElement | null>,
  scroller: RefObject<HTMLElement | null>,
  content: RefObject<HTMLElement | null>,
  listening: boolean,
  fail: (error: unknown) => void,
) {
  const reported = useRef(0);
  const holding = useRef(0);

  const needed = useCallback(() => {
    const [section, list, inner] = [panel.current, scroller.current, content.current];
    return section && list && inner ? section.offsetHeight - list.clientHeight + inner.offsetHeight : 0;
  }, [panel, scroller, content]);

  const report = useCallback(
    (height: number) => {
      if (height === reported.current) return;
      reported.current = height;
      setPanelHeight(height).catch(fail);
    },
    [fail],
  );

  useEffect(() => {
    const inner = content.current;
    if (!listening || !inner) return;
    let frame = 0;
    const settle = () => {
      frame = 0;
      const height = Math.max(needed(), holding.current);
      const running = shrinking(inner);
      if (height < reported.current && running.length > 0) {
        void Promise.allSettled(running.map((animation) => animation.finished)).then(schedule);
      } else {
        report(height);
      }
    };
    const schedule = () => {
      if (frame === 0) frame = requestAnimationFrame(settle);
    };
    const observer = new ResizeObserver(schedule);
    observer.observe(inner);
    return () => {
      observer.disconnect();
      cancelAnimationFrame(frame);
    };
  }, [listening, content, needed, report]);

  return useCallback(
    (extra = 0) => {
      const settle = (asked: number): Promise<void> => {
        const target = needed() + extra;
        if (target <= window.innerHeight || target === asked) return Promise.resolve();
        holding.current = Math.max(holding.current, target);
        report(target);
        const reachable = Math.min(target, window.screen.availHeight - SCREEN_ALLOWANCE_PX) - 1;
        return new Promise<void>((resolve) => {
          const done = () => {
            window.removeEventListener('resize', check);
            window.clearTimeout(timer);
            resolve();
          };
          const check = () => {
            if (window.innerHeight >= reachable) done();
          };
          const timer = window.setTimeout(done, FIT_TIMEOUT_MS);
          window.addEventListener('resize', check);
          check();
        }).then(() => settle(target));
      };
      return settle(0).finally(() => {
        holding.current = 0;
      });
    },
    [needed, report],
  );
}
