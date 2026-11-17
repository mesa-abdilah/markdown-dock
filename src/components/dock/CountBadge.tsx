import { cn } from "../../lib/cn";

export function CountBadge({
  count,
  variant = "pill",
  className,
}: {
  count: number;
  variant?: "pill" | "embedded";
  className?: string;
}) {
  const formatted = count > 999 ? "999+" : String(count);
  return (
    <span
      className={cn(
        variant === "embedded"
          ? "inline-flex min-w-[16px] items-center justify-center px-1 text-[10.5px] font-bold tracking-tight text-sky-100 tabular-nums drop-shadow-[0_1px_2px_rgba(0,0,0,0.65)]"
          : "inline-flex min-w-[22px] items-center justify-center rounded-full bg-gradient-to-b from-sky-400/25 to-blue-500/20 px-1.5 py-0.5 text-[10px] font-bold text-sky-100 tabular-nums ring-1 ring-sky-300/35 shadow-[inset_0_1px_0_rgba(255,255,255,0.18),0_2px_6px_rgba(14,165,233,0.18)]",
        className,
      )}
      aria-label={`${count} indexed markdown files`}
    >
      {formatted}
    </span>
  );
}
