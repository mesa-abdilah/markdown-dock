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
        "relative h-full w-full overflow-hidden text-zinc-100 shadow-[0_10px_28px_rgba(0,0,0,0.68),0_0_0_1px_rgba(255,255,255,0.12)] backdrop-blur-md backdrop-saturate-150",
        className,
      )}
      style={{
        transform: "translateZ(0)",
        backgroundImage: `linear-gradient(165deg, var(--dmd-glass-top) 0%, var(--dmd-glass-bottom) 100%)`,
        boxShadow:
          "inset 0 1px 0 rgba(255,255,255,0.18), inset 0 -1px 0 rgba(255,255,255,0.04), inset 0 0 0 1px rgba(255,255,255,0.08)",
      }}
    >
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0"
        style={{
          backgroundImage:
            "linear-gradient(135deg, rgba(255,255,255,0.09) 0%, rgba(255,255,255,0.02) 26%, transparent 52%)",
        }}
      />
      <div className="relative z-10 h-full w-full">{children}</div>
    </div>
  );
}
