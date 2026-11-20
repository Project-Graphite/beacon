const THUMB_INSETS_PX = 6;
const THUMB_MIN_PX = 24;

export function thumbFraction(viewport: number, content: number): number {
  if (content <= viewport) return 1;
  return Math.min(1, Math.max(viewport / content, THUMB_MIN_PX / (viewport - THUMB_INSETS_PX)));
}
