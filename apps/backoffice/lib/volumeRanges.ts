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
