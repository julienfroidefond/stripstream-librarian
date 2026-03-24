"use client";

interface ExternalLinkBadgeProps {
  href: string;
  className?: string;
  children: React.ReactNode;
}

export function ExternalLinkBadge({ href, className, children }: ExternalLinkBadgeProps) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className={className}
      onClick={(e) => e.stopPropagation()}
    >
      {children}
    </a>
  );
}
