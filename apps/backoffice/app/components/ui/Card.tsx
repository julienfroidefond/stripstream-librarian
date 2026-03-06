import { ReactNode } from "react";

interface CardProps {
  children: ReactNode;
  className?: string;
}

export function Card({ children, className = "" }: CardProps) {
  return (
    <div className={`bg-card rounded-xl shadow-soft border border-line p-6 ${className}`}>
      {children}
    </div>
  );
}

interface CardHeaderProps {
  title: string;
  className?: string;
}

export function CardHeader({ title, className = "" }: CardHeaderProps) {
  return (
    <h2 className={`text-lg font-semibold text-foreground mb-4 ${className}`}>
      {title}
    </h2>
  );
}
