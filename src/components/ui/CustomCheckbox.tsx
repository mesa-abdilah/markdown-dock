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
 * Custom styled checkbox / toggle switch backed by an accessible native checkbox input and theme tokens.
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
          style={{
            backgroundColor: checked ? "var(--accent-primary)" : "var(--bg-surface)",
            borderColor: "var(--border-color)",
            borderWidth: "var(--border-width)",
            borderStyle: "solid",
            boxShadow: checked ? "var(--shadow-button)" : "var(--shadow-inset)",
          }}
          className="relative inline-flex h-[18px] w-[32px] items-center rounded-full transition-all duration-200 peer-focus-visible:ring-2 peer-focus-visible:ring-[var(--accent-primary)]"
        >
          <span
            style={{
              backgroundColor: checked ? "var(--accent-foreground)" : "var(--text-secondary)",
            }}
            className={cn(
              "inline-block h-3.5 w-3.5 rounded-full shadow-sm transition-transform duration-150",
              checked ? "translate-x-[14px]" : "translate-x-[2px]",
            )}
          />
        </span>
      ) : (
        <span
          aria-hidden="true"
          style={{
            backgroundColor: checked ? "var(--accent-primary)" : "var(--bg-surface)",
            color: checked ? "var(--accent-foreground)" : "transparent",
            borderColor: "var(--border-color)",
            borderWidth: "var(--border-width)",
            borderStyle: "solid",
            borderRadius: "var(--radius-sm)",
            boxShadow: checked ? "var(--shadow-button)" : "var(--shadow-inset)",
          }}
          className="inline-flex h-4 w-4 items-center justify-center transition-all duration-200 peer-focus-visible:ring-2 peer-focus-visible:ring-[var(--accent-primary)]"
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
