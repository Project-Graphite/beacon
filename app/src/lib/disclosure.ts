export const toggleOpen = (open: readonly string[], id: string): string[] =>
  open.includes(id) ? open.filter((each) => each !== id) : [...open, id];
