const isAbsolute = (path: string) => /^(?:[a-z]:|[\\/])/i.test(path);

function segments(path: string): string[] {
  const resolved: string[] = [];
  for (const part of path.split(/[\\/]+/)) {
    if (part === '..') resolved.pop();
    else if (part !== '' && part !== '.') resolved.push(part);
  }
  return resolved;
}

export function relativeInside(path: string, cwd: string): string | null {
  const root = segments(cwd);
  const target = segments(isAbsolute(path) ? path : `${cwd}/${path}`);
  const inside =
    target.length >= root.length && root.every((part, index) => part.toLowerCase() === target[index].toLowerCase());
  return inside ? target.slice(root.length).join(path.includes('\\') || !path.includes('/') ? '\\' : '/') : null;
}

export function displayPath(path: string, cwd: string | null): string {
  return (cwd === null ? null : relativeInside(path, cwd)) || path;
}

export function shortenPath(path: string, maxChars: number): string {
  if (path.length <= maxChars) return path;
  const tail = path.slice(path.length - (maxChars - 1));
  const separator = tail.search(/[\\/]/);
  return `…${separator === -1 ? tail : tail.slice(separator)}`;
}
