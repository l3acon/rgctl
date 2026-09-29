/** Tester case A2 — two declarations; expect only `alpha2` and `beta` (no anonymous dupes). */
export function alpha2(n: number): number {
  return n + 1;
}

function beta(n: number): number {
  return alpha2(n);
}
