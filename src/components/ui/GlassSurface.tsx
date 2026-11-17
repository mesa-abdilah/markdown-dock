import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

export type AccentTone = "indigo" | "emerald" | "amber" | "rose";

const NOISE_SVG =
  "url(\"data:image/svg+xml,%3Csvg viewBox='0 0 120 120' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.03'/%3E%3C/svg%3E\")";

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
        "relative h-full w-full overflow-hidden text-zinc-100 shadow-[0_10px_28px_rgba(0,0,0,0.68),0_0_0_1px_rgba(255,255,255,0.12)] backdrop-blur-2xl backdrop-saturate-150",
        className,
      )}
      style={{
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
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0"
        style={{ backgroundImage: NOISE_SVG }}
      />
      <div className="relative z-10 h-full w-full">{children}</div>
    </div>
  );
}
