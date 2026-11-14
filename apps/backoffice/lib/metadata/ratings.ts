/** Convert a rating on any native provider scale to the internal 0-10 scale. */
export function normalizeRating(rating: number, scale: number): number {
  return (rating / scale) * 10;
}
