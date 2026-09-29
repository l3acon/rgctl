/** Tester case B (file one). */
export const dup = (n) => n + 1;
export const uniqueOne = (n) => dup(n);

export function dupDecl(n) {
  return n + 1;
}
export function uniqueDecl(n) {
  return dupDecl(n);
}
