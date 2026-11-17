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
 * Custom glassmorphic dropdown select rendered via Portal so it is never clipped by overflow containers.
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

    const minW =
      variant === "field" ? Math.max(rect.width, 160) : Math.max(rect.width, 156);

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
        className={cn(
          "inline-flex cursor-pointer items-center justify-between gap-1.5 outline-none transition-all focus-visible:ring-2 focus-visible:ring-indigo-400/70 disabled:cursor-not-allowed disabled:opacity-45",
          variant === "field" &&
            "h-8 w-full rounded-lg border border-white/12 bg-white/[0.06] px-2.5 text-xs text-zinc-100 shadow-[inset_0_1px_0_rgba(255,255,255,0.06)] hover:border-white/20 hover:bg-white/[0.09]",
          variant === "compact" &&
            "h-6 rounded-md border border-white/12 bg-white/[0.07] px-2 text-[10px] font-medium text-zinc-200 hover:border-white/20 hover:bg-white/[0.12]",
          variant === "action" &&
            (triggerLabel
              ? "h-7 rounded-lg border border-white/10 bg-white/[0.07] px-2 text-[11px] font-medium text-zinc-200 hover:border-indigo-400/35 hover:bg-indigo-500/20 hover:text-white"
              : "h-6 w-6 justify-center rounded-md bg-white/[0.07] text-zinc-300 hover:bg-indigo-500/25 hover:text-white"),
          open && "border-indigo-400/55 ring-2 ring-indigo-400/25",
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
                  "h-3 w-3 text-zinc-400 transition-transform duration-150",
                  open && "rotate-180 text-indigo-300",
                )}
              />
            )}
          </>
        ) : (
          <>
            <span className="flex min-w-0 items-center gap-1.5 truncate">
              {triggerIcon}
              <span className={cn("truncate", !selectedOption && "text-zinc-400")}>
                {selectedOption?.label ?? placeholder}
              </span>
            </span>
            <ChevronDown
              className={cn(
                "h-3.5 w-3.5 shrink-0 text-zinc-400 transition-transform duration-150",
                open && "rotate-180 text-indigo-300",
              )}
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
            }}
            className="z-[9999] max-h-60 overflow-y-auto rounded-xl border border-white/15 bg-zinc-900/95 p-1 shadow-[0_14px_36px_rgba(0,0,0,0.72),inset_0_1px_0_rgba(255,255,255,0.08)] backdrop-blur-xl"
          >
            {menuHeader && (
              <div className="px-2 py-1 text-[10px] font-semibold tracking-wide text-zinc-400 uppercase">
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
                    className={cn(
                      "flex w-full cursor-pointer items-center justify-between gap-2 rounded-lg px-2 py-1.5 text-left text-[11px] transition-colors",
                      isSelected
                        ? "bg-indigo-500/25 font-medium text-indigo-100"
                        : isHighlighted
                          ? "bg-white/10 text-white"
                          : "text-zinc-300 hover:bg-white/[0.08] hover:text-white",
                    )}
                  >
                    <span className="flex min-w-0 items-center gap-1.5">
                      {opt.icon && <span className="shrink-0 text-amber-300">{opt.icon}</span>}
                      <span className="truncate">{opt.label}</span>
                    </span>
                    {isSelected && <Check className="h-3 w-3 shrink-0 text-indigo-300" />}
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
