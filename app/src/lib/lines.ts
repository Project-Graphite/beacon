const LINE_LIMIT = 40;

export const textLines = (text: string) => text.replace(/\r?\n$/, '').split(/\r?\n/);

export const clampLines = <T>(lines: T[], expanded: boolean): T[] =>
  expanded || lines.length <= LINE_LIMIT ? lines : lines.slice(0, LINE_LIMIT);
