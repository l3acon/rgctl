/** Tester case B (file two). */
export const dup = (n) => n + 2;
export const uniqueTwo = (n) => dup(n);

export function dupDecl2(n) {
  return n + 2;
}
export function uniqueDecl2(n) {
  return dupDecl2(n);
}
