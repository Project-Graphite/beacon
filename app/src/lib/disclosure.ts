export const toggleOpen = <T extends string>(open: readonly T[], id: T): T[] =>
  open.includes(id) ? open.filter((each) => each !== id) : [...open, id];
