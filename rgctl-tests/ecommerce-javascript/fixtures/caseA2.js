/** Tester case A2 — two declarations; expect only `alpha2` and `beta`. */
export function alpha2(n) {
  return n + 1;
}

function beta(n) {
  return alpha2(n);
}
