const THUMB_INSETS_PX = 6;
const THUMB_MIN_PX = 24;

export function fullyShown(item: { top: number; bottom: number }, view: { top: number; bottom: number }): boolean {
  return (item.top >= view.top - 1 && item.bottom <= view.bottom + 1) || (item.top <= view.top && item.bottom >= view.bottom);
}

export function thumbFraction(viewport: number, content: number): number {
  if (content <= viewport) return 1;
  return Math.min(1, Math.max(viewport / content, THUMB_MIN_PX / (viewport - THUMB_INSETS_PX)));
}
