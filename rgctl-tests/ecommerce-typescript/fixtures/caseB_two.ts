/** Tester case B (file two) — second `dup` for path::symbol disambiguation. */
export const dup = (n: number): number => n + 2;
export const uniqueTwo = (n: number): number => dup(n);

export function dupDecl2(n: number): number {
  return n + 2;
}
export function uniqueDecl2(n: number): number {
  return dupDecl2(n);
}
