const LEADING_ARTICLES = ["L'", "L' ", "Le ", "La ", "Les ", "Un ", "Une ", "Des ", "The ", "A ", "An "];

/**
 * Strip a leading French/English article from a series name.
 * Returns the stripped string, or the original if no article was found.
 * e.g. "L'Atelier des sorciers" → "Atelier des sorciers"
 *      "Les Géants" → "Géants"
 */
export function stripLeadingArticle(name: string): string {
  for (const article of LEADING_ARTICLES) {
    if (name.toLowerCase().startsWith(article.toLowerCase())) {
      return name.slice(article.length).trim();
    }
  }
  return name;
}

/**
 * Compress a sorted list of volume numbers into ranges.
 * e.g. [1,2,3,5,7,8,9] → ["1→3", "5", "7→9"]
 */
export function compressVolumes(volumes: number[]): string[] {
  if (volumes.length === 0) return [];

  const sorted = [...volumes].sort((a, b) => a - b);
  const ranges: string[] = [];
  let start = sorted[0];
  let end = sorted[0];

  for (let i = 1; i < sorted.length; i++) {
    if (sorted[i] === end + 1) {
      end = sorted[i];
    } else {
      ranges.push(start === end ? `T${start}` : `T${start}→${end}`);
      start = sorted[i];
      end = sorted[i];
    }
  }
  ranges.push(start === end ? `T${start}` : `T${start}→${end}`);

  return ranges;
}
