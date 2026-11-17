import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

export function Kbd({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <kbd
      className={cn(
        "inline-flex items-center rounded border border-white/15 bg-white/[0.06] px-1.5 py-0.5 font-mono text-[10px] leading-none font-medium text-zinc-300",
        className,
      )}
    >
      {children}
    </kbd>
  );
}
