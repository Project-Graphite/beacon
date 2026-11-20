import { useRef, useState, type ReactNode, type RefObject } from 'react';
import { useViewTransition, type View } from '../hooks/useViewTransition.ts';

interface ViewStackProps {
  view: View;
  scroller: RefObject<HTMLElement | null>;
  render: (key: string, active: boolean) => ReactNode;
}

export function ViewStack({ view, scroller, render }: ViewStackProps) {
  const [views, setViews] = useState([view]);
  const shown = views[views.length - 1];
  if (shown.key !== view.key) setViews([shown, view]);
  const stack = useRef<HTMLDivElement>(null);
  useViewTransition(stack, scroller, views, setViews);
  return (
    <div ref={stack} className="grid">
      {views.map((each) => (
        <div key={each.key} inert={each !== shown} className="col-start-1 row-start-1 min-w-0">
          {render(each.key, each === shown)}
        </div>
      ))}
    </div>
  );
}
