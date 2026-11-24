/** Short, pairwise-distinct chip labels for workspace names (design D11, R7). */

function firstChar(name: string): string {
  const c = Array.from(name)[0];
  return c ? c.toUpperCase() : "?";
}

function initials(name: string): string {
  const words = name.split(/[-_. ]+/).filter((w) => w.length > 0);
  const letters = words.slice(0, 2).map((w) => firstChar(w));
  return letters.length > 0 ? letters.join("") : firstChar(name);
}

function firstTwo(name: string): string {
  return Array.from(name).slice(0, 2).join("").toUpperCase() || "?";
}

/** The indexes whose label equals another's. */
function colliding(labels: string[]): number[] {
  return labels.map((_, i) => i).filter((i) => labels.some((o, j) => j !== i && o === labels[i]));
}

export function chipLabels(names: string[]): string[] {
  const tiers = [firstChar, initials, firstTwo];
  const labels = names.map((n) => tiers[0](n));
  for (let t = 1; t < tiers.length; t++) {
    for (const i of colliding(labels)) labels[i] = tiers[t](names[i]);
  }
  // Still colliding: the first letter and a 1-based position among the stuck
  // names that share that first letter, so the numbers never repeat.
  const counts = new Map<string, number>();
  for (const i of colliding(labels)) {
    const letter = firstChar(names[i]);
    const n = (counts.get(letter) ?? 0) + 1;
    counts.set(letter, n);
    labels[i] = `${letter}${n}`;
  }
  return labels;
}
