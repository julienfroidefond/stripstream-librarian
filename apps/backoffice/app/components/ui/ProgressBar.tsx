interface ProgressBarProps {
  value: number;
  max?: number;
  showLabel?: boolean;
  size?: "sm" | "md" | "lg";
  className?: string;
}

const sizeStyles = {
  sm: "h-1.5",
  md: "h-2",
  lg: "h-8",
};

export function ProgressBar({ 
  value, 
  max = 100, 
  showLabel = false, 
  size = "md",
  className = "" 
}: ProgressBarProps) {
  const percent = Math.min(100, Math.max(0, (value / max) * 100));
  
  return (
    <div className={`relative ${sizeStyles[size]} bg-line rounded-full overflow-hidden ${className}`}>
      <div 
        className="absolute inset-y-0 left-0 bg-success rounded-full transition-all duration-300"
        style={{ width: `${percent}%` }}
      />
      {showLabel && (
        <span className="absolute inset-0 flex items-center justify-center text-sm font-semibold text-foreground">
          {Math.round(percent)}%
        </span>
      )}
    </div>
  );
}

interface MiniProgressBarProps {
  value: number;
  max?: number;
  className?: string;
}

export function MiniProgressBar({ value, max = 100, className = "" }: MiniProgressBarProps) {
  const percent = Math.min(100, Math.max(0, (value / max) * 100));
  
  return (
    <div className={`flex-1 h-1.5 bg-line rounded-full overflow-hidden ${className}`}>
      <div 
        className="h-full bg-success rounded-full transition-all duration-300"
        style={{ width: `${percent}%` }}
      />
    </div>
  );
}
