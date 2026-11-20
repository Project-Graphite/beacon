import { useContext, useLayoutEffect, useRef, type Dispatch, type RefObject, type SetStateAction } from 'react';
import { easing, play } from '../motion.ts';
import { PanelFit } from './usePanelHeight.ts';

export interface View {
  key: string;
  depth: number;
}

export function useViewTransition(
  stack: RefObject<HTMLElement | null>,
  scroller: RefObject<HTMLElement | null>,
  views: View[],
  setViews: Dispatch<SetStateAction<View[]>>,
) {
  const fit = useContext(PanelFit);
  const listScroll = useRef(0);

  useLayoutEffect(() => {
    const [root, list] = [stack.current, scroller.current];
    if (views.length < 2 || !root || !list) return;
    const [from, to] = views;
    const [leaving, entering] = root.children;
    const before = list.scrollTop;
    if (from.depth === 0) listScroll.current = before;
    list.scrollTop = to.depth === 0 ? listScroll.current : 0;
    const shift = list.scrollTop - before;
    const direction = Math.sign(to.depth - from.depth);
    const timing = { duration: direction === 0 ? 140 : 200, easing: easing('settle') };
    const settle = () => setViews((current) => current.slice(-1));
    const enter = play(
      entering,
      direction === 0
        ? [{ opacity: 0 }, { opacity: 1 }]
        : [{ transform: `translateX(${direction * 100}%)` }, { transform: 'none' }],
      { ...timing, fill: 'backwards' },
    );
    const leave = play(
      leaving,
      direction === 0
        ? [
            { opacity: 1, transform: `translateY(${shift}px)` },
            { opacity: 0, transform: `translateY(${shift}px)` },
          ]
        : [{ transform: `translateY(${shift}px)` }, { transform: `translate(${-direction * 100}%, ${shift}px)` }],
      { ...timing, fill: 'forwards' },
    );
    if (!enter || !leave) {
      settle();
      return;
    }
    let current = true;
    enter.pause();
    leave.pause();
    void fit().then(() => {
      if (!current) return;
      enter.play();
      leave.play();
    });
    leave.finished.then(
      () => current && settle(),
      () => {},
    );
    return () => {
      current = false;
      enter.cancel();
      leave.cancel();
    };
  }, [stack, scroller, views, setViews, fit]);
}
