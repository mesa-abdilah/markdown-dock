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
      style={
        variant === "embedded"
          ? { color: "var(--text-primary)" }
          : {
              backgroundColor: "var(--bg-surface)",
              color: "var(--text-primary)",
              borderColor: "var(--border-color)",
              borderWidth: "var(--border-width)",
              borderStyle: "solid",
              boxShadow: "var(--shadow-button)",
            }
      }
      className={cn(
        variant === "embedded"
          ? "inline-flex min-w-[16px] items-center justify-center px-1 text-[10.5px] font-bold tracking-tight tabular-nums"
          : "inline-flex min-w-[22px] items-center justify-center rounded-full px-1.5 py-0.5 text-[10px] font-bold tabular-nums",
        className,
      )}
      aria-label={`${count} indexed markdown files`}
    >
      {formatted}
    </span>
  );
}
