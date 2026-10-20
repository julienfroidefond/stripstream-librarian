interface RefreshIconProps {
  spinning: boolean;
  className?: string;
}

export const RefreshIcon = ({ spinning, className = "" }: RefreshIconProps) => (
  <svg
    className={`w-4 h-4 ${spinning ? "animate-spin" : ""} ${className}`}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={2}
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
    <path d="M21 3v5h-5" />
    <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
    <path d="M3 21v-5h5" />
  </svg>
);
