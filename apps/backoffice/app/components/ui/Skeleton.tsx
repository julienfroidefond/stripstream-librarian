interface SkeletonProps {
  className?: string;
}

/**
 * Shimmering placeholder shown while a server component streams in.
 *
 * Base look (`skeleton-shimmer rounded-md bg-muted`); the caller supplies the
 * shape/size through `className` (e.g. `h-20 rounded-xl`, `h-4 w-1/3`).
 */
export function Skeleton({ className = "" }: SkeletonProps) {
  return (
    <div
      aria-hidden="true"
      className={`relative skeleton-shimmer rounded-md bg-muted ${className}`}
    />
  );
}
