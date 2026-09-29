/** Tester case B (file one) — bound arrows must have CFG; `dup` is intentionally ambiguous with caseB_two. */
export const dup = (n: number): number => n + 1;
export const uniqueOne = (n: number): number => dup(n);

export function dupDecl(n: number): number {
  return n + 1;
}
export function uniqueDecl(n: number): number {
  return dupDecl(n);
}
