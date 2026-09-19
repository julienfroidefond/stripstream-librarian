import type { ReactNode } from "react";

type CoverFanProps = {
  background: ReactNode;
  covers: ReactNode[];
  className?: string;
};

/** Shared cover fan used by collection cards. */
export function CoverFan({ background, covers, className = "" }: CoverFanProps) {
  const visibleCovers = covers;
  const middle = (visibleCovers.length - 1) / 2;

  return (
    <div className={`relative w-full overflow-hidden bg-muted/10 ${className}`}>
      <div className="absolute inset-0">{background}</div>
      <div className="absolute inset-0 flex items-end justify-center">
        {visibleCovers.map((cover, index) => {
          const angle = (index - middle) * 12;
          const radius = 220;
          const radians = ((angle - 90) * Math.PI) / 180;
          const x = Math.cos(radians) * radius;
          const y = Math.sin(radians) * radius;
          return (
            <div
              key={index}
              className="absolute h-36 w-24 shadow-lg"
              style={{
                transform: `translate(${x}px, ${y}px) rotate(${angle}deg)`,
                transformOrigin: "bottom center",
                zIndex: visibleCovers.length - Math.abs(Math.round(index - middle)),
                bottom: "-185px",
              }}
            >
              {cover}
            </div>
          );
        })}
      </div>
    </div>
  );
}
