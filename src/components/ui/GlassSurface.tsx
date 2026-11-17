import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

export type AccentTone = "indigo" | "emerald" | "amber" | "rose";

interface GlassSurfaceProps {
  children: ReactNode;
  className?: string;
  tone?: AccentTone;
}

export function GlassSurface({ children, className, tone = "indigo" }: GlassSurfaceProps) {
  void tone;
  return (
    <div
      className={cn(
        "dmd-theme-shell relative h-full w-full overflow-hidden",
        className,
      )}
      style={{
        transform: "translateZ(0)",
      }}
    >
      <div className="relative z-10 h-full w-full">{children}</div>
    </div>
  );
}
