interface ProgressBarProps {
  value: number;
  max?: number;
  showLabel?: boolean;
  size?: "sm" | "md" | "lg";
  variant?: "default" | "success" | "warning" | "error";
  className?: string;
}

const sizeStyles = {
  sm: "h-1.5",
  md: "h-2",
  lg: "h-4",
};

const variantStyles = {
  default: "bg-primary",
  success: "bg-success",
  warning: "bg-warning",
  error: "bg-destructive",
};

export function ProgressBar({ 
  value, 
  max = 100, 
  showLabel = false, 
  size = "md",
  variant = "default",
  className = "" 
}: ProgressBarProps) {
  const percent = Math.min(100, Math.max(0, (value / max) * 100));
  
  return (
    <div className={`relative ${sizeStyles[size]} bg-muted/50 rounded-full overflow-hidden ${className}`}>
      <div 
        className={`absolute inset-y-0 left-0 rounded-full transition-all duration-500 ease-out ${variantStyles[variant]}`}
        style={{ width: `${percent}%` }}
      />
      {showLabel && (
        <span className="absolute inset-0 flex items-center justify-center text-xs font-semibold text-foreground">
          {Math.round(percent)}%
        </span>
      )}
    </div>
  );
}

// Mini Progress Bar (for compact displays)
interface MiniProgressBarProps {
  value: number;
  max?: number;
  variant?: "default" | "success" | "warning" | "error";
  className?: string;
}

export function MiniProgressBar({ 
  value, 
  max = 100, 
  variant = "default",
  className = "" 
}: MiniProgressBarProps) {
  const percent = Math.min(100, Math.max(0, (value / max) * 100));
  
  return (
    <div className={`flex-1 h-1.5 bg-muted/50 rounded-full overflow-hidden ${className}`}>
      <div 
        className={`h-full rounded-full transition-all duration-500 ease-out ${variantStyles[variant]}`}
        style={{ width: `${percent}%` }}
      />
    </div>
  );
}

// Progress indicator with status colors based on percentage
interface SmartProgressBarProps {
  value: number;
  max?: number;
  size?: "sm" | "md" | "lg";
  className?: string;
}

export function SmartProgressBar({ 
  value, 
  max = 100, 
  size = "md",
  className = "" 
}: SmartProgressBarProps) {
  const percent = Math.min(100, Math.max(0, (value / max) * 100));
  
  // Determine variant based on percentage
  let variant: "default" | "success" | "warning" | "error" = "default";
  if (percent === 100) variant = "success";
  else if (percent < 25) variant = "error";
  else if (percent < 50) variant = "warning";
  
  return <ProgressBar value={value} max={max} size={size} variant={variant} className={className} />;
}

// Circular Progress (for special use cases)
interface CircularProgressProps {
  value: number;
  max?: number;
  size?: number;
  strokeWidth?: number;
  className?: string;
}

export function CircularProgress({ 
  value, 
  max = 100, 
  size = 40, 
  strokeWidth = 4,
  className = "" 
}: CircularProgressProps) {
  const percent = Math.min(100, Math.max(0, (value / max) * 100));
  const radius = (size - strokeWidth) / 2;
  const circumference = radius * 2 * Math.PI;
  const offset = circumference - (percent / 100) * circumference;
  
  // Determine color based on percentage
  let color = "hsl(var(--color-primary))";
  if (percent === 100) color = "hsl(var(--color-success))";
  else if (percent < 25) color = "hsl(var(--color-destructive))";
  else if (percent < 50) color = "hsl(var(--color-warning))";
  
  return (
    <div className={`relative inline-flex items-center justify-center ${className}`} style={{ width: size, height: size }}>
      <svg className="transform -rotate-90" width={size} height={size}>
        <circle
          className="text-muted-foreground"
          stroke="currentColor"
          fill="transparent"
          strokeWidth={strokeWidth}
          r={radius}
          cx={size / 2}
          cy={size / 2}
        />
        <circle
          stroke={color}
          fill="transparent"
          strokeWidth={strokeWidth}
          strokeLinecap="round"
          strokeDasharray={circumference}
          strokeDashoffset={offset}
          r={radius}
          cx={size / 2}
          cy={size / 2}
          className="transition-all duration-500 ease-out"
        />
      </svg>
      <span className="absolute text-xs font-semibold text-foreground">
        {Math.round(percent)}%
      </span>
    </div>
  );
}
