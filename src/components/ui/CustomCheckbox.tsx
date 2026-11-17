import { Check } from "lucide-react";
import { cn } from "../../lib/cn";

interface CustomCheckboxProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  ariaLabel?: string;
  disabled?: boolean;
  variant?: "box" | "switch";
  className?: string;
}

/**
 * Custom styled checkbox / toggle switch backed by an accessible native checkbox input.
 */
export function CustomCheckbox({
  checked,
  onChange,
  ariaLabel,
  disabled = false,
  variant = "box",
  className,
}: CustomCheckboxProps) {
  return (
    <span
      className={cn(
        "relative inline-flex shrink-0 items-center justify-center",
        disabled && "cursor-not-allowed opacity-45",
        className,
      )}
    >
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        aria-label={ariaLabel}
        onChange={(e) => onChange(e.target.checked)}
        className="peer sr-only"
      />
      {variant === "switch" ? (
        <span
          aria-hidden="true"
          className={cn(
            "relative inline-flex h-[18px] w-[32px] items-center rounded-full border transition-all duration-150 peer-focus-visible:ring-2 peer-focus-visible:ring-indigo-400/70",
            checked
              ? "border-indigo-400/50 bg-gradient-to-r from-indigo-500 to-sky-500 shadow-[0_0_10px_rgba(99,102,241,0.35)]"
              : "border-white/15 bg-white/[0.08] hover:border-white/25 hover:bg-white/[0.12]",
          )}
        >
          <span
            className={cn(
              "inline-block h-3.5 w-3.5 rounded-full bg-white shadow-sm transition-transform duration-150",
              checked ? "translate-x-[15px]" : "translate-x-[2px] opacity-80",
            )}
          />
        </span>
      ) : (
        <span
          aria-hidden="true"
          className={cn(
            "inline-flex h-4 w-4 items-center justify-center rounded-[5px] border transition-all duration-150 peer-focus-visible:ring-2 peer-focus-visible:ring-indigo-400/70",
            checked
              ? "border-sky-300/60 bg-gradient-to-b from-indigo-500 to-sky-500 text-white shadow-[0_2px_8px_rgba(56,189,248,0.35)]"
              : "border-white/20 bg-white/[0.06] text-transparent hover:border-white/35 hover:bg-white/[0.1]",
          )}
        >
          <Check
            className={cn(
              "h-2.5 w-2.5 transition-transform duration-150",
              checked ? "scale-100 opacity-100" : "scale-50 opacity-0",
            )}
            strokeWidth={3}
          />
        </span>
      )}
    </span>
  );
}
