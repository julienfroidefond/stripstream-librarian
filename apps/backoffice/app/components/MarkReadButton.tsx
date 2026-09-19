"use client";

interface MarkReadButtonProps {
  label: string;
  loading: boolean;
  compact?: boolean;
  /** Whether the item is already marked as read (controls which icon is shown) */
  completed: boolean;
  className: string;
  onClick: (e: React.MouseEvent) => void;
}

export function MarkReadButton({ label, loading, compact, completed, className, onClick }: MarkReadButtonProps) {
  const size = compact ? "w-3.5 h-3.5" : "w-4 h-4";

  return (
    <button
      type="button"
      onClick={onClick}
      disabled={loading}
      title={label}
      className={`inline-flex items-center gap-1.5 transition-colors disabled:opacity-50 ${className}`}
    >
      {loading ? (
        <svg className={`${size} animate-spin`} fill="none" viewBox="0 0 24 24">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
        </svg>
      ) : completed ? (
        <svg className={size} fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M9 15 3 9m0 0 6-6M3 9h12a6 6 0 0 1 0 12h-3" />
        </svg>
      ) : (
        <svg className={size} fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0z" />
        </svg>
      )}
      {label}
    </button>
  );
}
