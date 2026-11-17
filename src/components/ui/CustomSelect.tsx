import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown } from "lucide-react";
import { cn } from "../../lib/cn";

export interface CustomSelectOption {
  value: string;
  label: string;
  description?: string;
  icon?: React.ReactNode;
}

interface CustomSelectProps {
  value: string;
  options: CustomSelectOption[];
  onChange: (value: string) => void;
  placeholder?: string;
  menuHeader?: string;
  ariaLabel?: string;
  title?: string;
  variant?: "field" | "compact" | "action";
  triggerIcon?: React.ReactNode;
  triggerLabel?: string;
  align?: "left" | "right";
  className?: string;
  disabled?: boolean;
}

interface MenuCoords {
  top?: number;
  bottom?: number;
  left?: number;
  right?: number;
  minWidth: number;
}

/**
 * Custom dropdown select rendered via Portal and styled with Multi-Theme CSS Variables.
 */
export function CustomSelect({
  value,
  options,
  onChange,
  placeholder = "Select...",
  menuHeader,
  ariaLabel,
  title,
  variant = "field",
  triggerIcon,
  triggerLabel,
  align = "left",
  className,
  disabled = false,
}: CustomSelectProps) {
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<MenuCoords | null>(null);
  const [highlightedIdx, setHighlightedIdx] = useState<number>(-1);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const listboxId = useId();

  const selectedOption = options.find((o) => o.value === value);

  const updatePosition = useCallback(() => {
    const trigger = triggerRef.current;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const spaceBelow = window.innerHeight - rect.bottom;
    const estMenuHeight = Math.min(240, options.length * 32 + (menuHeader ? 28 : 12));
    const openUpward = spaceBelow < estMenuHeight && rect.top > spaceBelow;

    const minW = variant === "field" ? Math.max(rect.width, 160) : Math.max(rect.width, 156);

    const next: MenuCoords = {
      minWidth: minW,
      ...(openUpward
        ? { bottom: window.innerHeight - rect.top + 6 }
        : { top: rect.bottom + 6 }),
    };

    if (align === "right" || rect.left + minW > window.innerWidth - 8) {
      next.right = Math.max(8, window.innerWidth - rect.right);
    } else {
      next.left = Math.max(8, rect.left);
    }

    setCoords(next);
  }, [align, menuHeader, options.length, variant]);

  useLayoutEffect(() => {
    if (!open) return;
    updatePosition();
  }, [open, updatePosition]);

  useEffect(() => {
    if (!open) return;

    const onPointerDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (triggerRef.current?.contains(target) || menuRef.current?.contains(target)) {
        return;
      }
      setOpen(false);
    };

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        setOpen(false);
        triggerRef.current?.focus();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setHighlightedIdx((prev) => (options.length === 0 ? -1 : (prev + 1) % options.length));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setHighlightedIdx((prev) =>
          options.length === 0 ? -1 : (prev - 1 + options.length) % options.length,
        );
      } else if (e.key === "Enter" && highlightedIdx >= 0 && highlightedIdx < options.length) {
        e.preventDefault();
        const chosen = options[highlightedIdx];
        if (chosen) {
          onChange(chosen.value);
          setOpen(false);
          triggerRef.current?.focus();
        }
      }
    };

    const onViewportChange = (e: Event) => {
      if (e.type === "scroll" && menuRef.current?.contains(e.target as Node)) {
        return;
      }
      setOpen(false);
    };

    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", onViewportChange);
    window.addEventListener("scroll", onViewportChange, true);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", onViewportChange);
      window.removeEventListener("scroll", onViewportChange, true);
    };
  }, [highlightedIdx, onChange, open, options]);

  const toggleOpen = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (disabled) return;
    setOpen((prev) => {
      const next = !prev;
      if (next) {
        const idx = options.findIndex((o) => o.value === value);
        setHighlightedIdx(idx >= 0 ? idx : 0);
      }
      return next;
    });
  };

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        role="combobox"
        aria-expanded={open}
        aria-controls={open ? listboxId : undefined}
        aria-label={ariaLabel}
        title={title}
        disabled={disabled}
        onClick={toggleOpen}
        style={{
          backgroundColor: "var(--bg-surface)",
          color: "var(--text-primary)",
          borderColor: open ? "var(--accent-primary)" : "var(--border-color)",
          borderWidth: "var(--border-width)",
          borderStyle: "solid",
          borderRadius: variant === "field" ? "var(--radius-md)" : "var(--radius-sm)",
          boxShadow: variant === "field" ? "var(--shadow-inset)" : "var(--shadow-button)",
        }}
        className={cn(
          "dmd-interactive-btn inline-flex cursor-pointer items-center justify-between gap-1.5 outline-none transition-all disabled:cursor-not-allowed disabled:opacity-45",
          variant === "field" && "h-8 w-full px-2.5 text-xs",
          variant === "compact" && "h-6 px-2 text-[10px] font-medium",
          variant === "action" &&
            (triggerLabel
              ? "h-7 px-2 text-[11px] font-medium"
              : "h-6 w-6 justify-center"),
          className,
        )}
      >
        {variant === "action" ? (
          <>
            {triggerIcon}
            {triggerLabel && <span>{triggerLabel}</span>}
            {triggerLabel && (
              <ChevronDown
                className={cn(
                  "h-3 w-3 transition-transform duration-150",
                  open && "rotate-180",
                )}
                style={{ color: "var(--text-secondary)" }}
              />
            )}
          </>
        ) : (
          <>
            <span className="flex min-w-0 items-center gap-1.5 truncate">
              {triggerIcon}
              <span
                className="truncate"
                style={{ color: selectedOption ? "var(--text-primary)" : "var(--text-muted)" }}
              >
                {selectedOption?.label ?? placeholder}
              </span>
            </span>
            <ChevronDown
              className={cn(
                "h-3.5 w-3.5 shrink-0 transition-transform duration-150",
                open && "rotate-180",
              )}
              style={{ color: "var(--text-secondary)" }}
            />
          </>
        )}
      </button>

      {open &&
        coords &&
        typeof document !== "undefined" &&
        createPortal(
          <div
            ref={menuRef}
            id={listboxId}
            role="listbox"
            aria-label={ariaLabel ?? menuHeader ?? "Options"}
            onClick={(e) => e.stopPropagation()}
            style={{
              position: "fixed",
              top: coords.top,
              bottom: coords.bottom,
              left: coords.left,
              right: coords.right,
              minWidth: coords.minWidth,
              maxWidth: 280,
              backgroundColor: "var(--bg-canvas)",
              backgroundImage: "var(--bg-canvas-image)",
              borderColor: "var(--border-color)",
              borderWidth: "var(--border-width)",
              borderStyle: "solid",
              borderRadius: "var(--radius-lg)",
              boxShadow: "var(--shadow-card)",
              backdropFilter: "var(--backdrop-blur)",
              WebkitBackdropFilter: "var(--backdrop-blur)",
              color: "var(--text-primary)",
            }}
            className="z-[9999] max-h-60 overflow-y-auto p-1 transition-all duration-200"
          >
            {menuHeader && (
              <div
                className="px-2 py-1 text-[10px] font-semibold tracking-wide uppercase"
                style={{ color: "var(--text-muted)" }}
              >
                {menuHeader}
              </div>
            )}
            <div className="space-y-0.5">
              {options.map((opt, idx) => {
                const isSelected = opt.value === value;
                const isHighlighted = idx === highlightedIdx;
                return (
                  <button
                    key={opt.value}
                    type="button"
                    role="option"
                    aria-selected={isSelected}
                    onMouseEnter={() => setHighlightedIdx(idx)}
                    onClick={(e) => {
                      e.stopPropagation();
                      onChange(opt.value);
                      setOpen(false);
                      triggerRef.current?.focus();
                    }}
                    style={{
                      backgroundColor: isSelected
                        ? "var(--accent-primary)"
                        : isHighlighted
                          ? "var(--bg-surface-hover)"
                          : "transparent",
                      color: isSelected ? "var(--accent-foreground)" : "var(--text-primary)",
                      borderRadius: "var(--radius-sm)",
                    }}
                    className="flex w-full cursor-pointer items-center justify-between gap-2 px-2 py-1.5 text-left text-[11px] font-medium transition-colors"
                  >
                    <span className="flex min-w-0 items-center gap-1.5">
                      {opt.icon && <span className="shrink-0">{opt.icon}</span>}
                      <span className="truncate">{opt.label}</span>
                    </span>
                    {isSelected && <Check className="h-3 w-3 shrink-0" />}
                  </button>
                );
              })}
            </div>
          </div>,
          document.body,
        )}
    </>
  );
}
