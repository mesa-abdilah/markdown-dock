import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, Palette, Pipette, RotateCcw } from "lucide-react";
import { cn } from "../../lib/cn";
import { PEEK_COLOR_PRESETS, useTheme, type ThemeMeta } from "../../lib/themes";

function ThemeMiniSwatch({ meta, size = "md" }: { meta: ThemeMeta; size?: "sm" | "md" }) {
  const { swatch } = meta;
  const isSm = size === "sm";
  return (
    <span
      aria-hidden="true"
      style={{
        background: swatch.canvas,
        borderRadius: isSm ? "5px" : "8px",
        border: "1px solid rgba(128,128,128,0.22)",
      }}
      className={cn(
        "relative inline-flex shrink-0 items-center justify-center overflow-hidden",
        isSm ? "h-4 w-5 p-0.5" : "h-8 w-11 p-1",
      )}
    >
      <span
        style={{
          background: swatch.surface,
          borderColor: swatch.border,
          borderWidth: swatch.borderWidth,
          borderStyle: "solid",
          borderRadius: swatch.radius,
          boxShadow: swatch.shadow,
          clipPath: swatch.clipPath,
        }}
        className="flex h-full w-full items-center justify-between px-1"
      >
        <span
          style={{ background: swatch.accent }}
          className={cn("rounded-full", isSm ? "h-1.5 w-1.5" : "h-2 w-2")}
        />
        {!isSm && (
          <span
            style={{ background: swatch.secondaryAccent ?? swatch.text, opacity: 0.65 }}
            className="h-1 w-3 rounded-full"
          />
        )}
      </span>
    </span>
  );
}

function PeekColorPickerControl() {
  const { peekColor, setPeekColor } = useTheme();
  const activeHex = peekColor ?? "#38bdf8";

  return (
    <div
      style={{
        backgroundColor: "var(--bg-surface)",
        borderColor: "var(--border-color)",
        borderWidth: "var(--border-width)",
        borderStyle: "solid",
        borderRadius: "var(--radius-md)",
        boxShadow: "var(--shadow-button)",
      }}
      className="space-y-2 p-2"
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-1.5">
          <Pipette className="h-3 w-3 shrink-0" style={{ color: "var(--text-secondary)" }} />
          <span className="text-[11px] font-semibold" style={{ color: "var(--text-primary)" }}>
            Dock Peek Color
          </span>
        </div>

        <div className="flex items-center gap-1">
          <label
            title="Pick custom color for Dock Peek sliver & capsule"
            style={{
              backgroundColor: "var(--bg-canvas)",
              borderColor: peekColor ?? "var(--border-color)",
              borderWidth: "var(--border-width)",
              borderStyle: "solid",
              borderRadius: "var(--radius-sm)",
            }}
            className="relative inline-flex h-6 cursor-pointer items-center gap-1.5 px-2 text-[10px] font-mono font-semibold"
          >
            <span
              aria-hidden="true"
              style={{
                backgroundColor: activeHex,
                boxShadow: peekColor ? `0 0 6px ${activeHex}` : undefined,
              }}
              className="h-3 w-3 rounded-full border border-black/20"
            />
            <span style={{ color: "var(--text-primary)" }}>
              {peekColor ? peekColor.toUpperCase() : "Custom"}
            </span>
            <input
              type="color"
              aria-label="Custom Dock Peek color picker"
              value={activeHex}
              onChange={(e) => setPeekColor(e.target.value)}
              className="sr-only"
            />
          </label>

          {peekColor && (
            <button
              type="button"
              onClick={() => setPeekColor(null)}
              title="Reset to automatic status color"
              aria-label="Reset Dock Peek color to auto"
              style={{
                backgroundColor: "var(--bg-canvas)",
                color: "var(--text-secondary)",
                borderColor: "var(--border-color)",
                borderWidth: "var(--border-width)",
                borderStyle: "solid",
                borderRadius: "var(--radius-sm)",
              }}
              className="inline-flex h-6 items-center gap-1 px-1.5 text-[10px] font-medium"
            >
              <RotateCcw className="h-2.5 w-2.5" />
              <span>Auto</span>
            </button>
          )}
        </div>
      </div>

      <div className="flex items-center justify-between gap-1 pt-0.5">
        <div className="flex items-center gap-1.5">
          {PEEK_COLOR_PRESETS.map((preset) => {
            const isSelected = peekColor?.toLowerCase() === preset.value.toLowerCase();
            return (
              <button
                key={preset.value}
                type="button"
                title={preset.label}
                aria-label={`Set Dock Peek color to ${preset.label}`}
                aria-pressed={isSelected}
                onClick={() => setPeekColor(preset.value)}
                style={{
                  backgroundColor: preset.value,
                  boxShadow: isSelected ? `0 0 8px ${preset.value}` : undefined,
                }}
                className={cn(
                  "h-4 w-4 cursor-pointer rounded-full border transition-transform hover:scale-110",
                  isSelected ? "scale-110 border-white ring-2 ring-black/40" : "border-black/20",
                )}
              />
            );
          })}
        </div>
        <span className="text-[9.5px]" style={{ color: "var(--text-muted)" }}>
          {peekColor ? "Custom" : "Auto status"}
        </span>
      </div>
    </div>
  );
}

interface ThemeSwitcherProps {
  mode?: "popover" | "grid";
  className?: string;
}

export function ThemeSwitcher({ mode = "popover", className }: ThemeSwitcherProps) {
  const { theme, themeMeta, themes, setTheme } = useTheme();
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<{ top?: number; bottom?: number; right: number } | null>(
    null,
  );
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuId = useId();

  const updatePosition = useCallback(() => {
    const el = triggerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const spaceBelow = window.innerHeight - rect.bottom;
    const estH = 420;
    const openUp = spaceBelow < estH && rect.top > spaceBelow;
    setCoords({
      right: Math.max(8, window.innerWidth - rect.right),
      ...(openUp ? { bottom: window.innerHeight - rect.top + 6 } : { top: rect.bottom + 6 }),
    });
  }, []);

  useLayoutEffect(() => {
    if (!open || mode !== "popover") return;
    updatePosition();
  }, [open, mode, updatePosition]);

  useEffect(() => {
    if (!open || mode !== "popover") return;

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
      }
    };

    const onResize = () => setOpen(false);

    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", onResize);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", onResize);
    };
  }, [open, mode]);

  if (mode === "grid") {
    return (
      <div className={cn("space-y-2.5", className)}>
        <div className="grid grid-cols-2 gap-2" role="radiogroup" aria-label="Theme">
          {themes.map((item) => {
            const active = item.id === theme;
            return (
              <button
                key={item.id}
                type="button"
                role="radio"
                aria-checked={active}
                onClick={() => setTheme(item.id)}
                className={cn(
                  "dmd-interactive-btn flex cursor-pointer items-center gap-2 p-2 text-left transition-all",
                  active && "ring-2 ring-[var(--accent-primary)]",
                )}
                style={{
                  backgroundColor: "var(--bg-surface)",
                  borderColor: active ? "var(--accent-primary)" : "var(--border-color)",
                  borderWidth: "var(--border-width)",
                  borderStyle: "solid",
                  borderRadius: "var(--radius-md)",
                  boxShadow: "var(--shadow-button)",
                  color: "var(--text-primary)",
                }}
              >
                <ThemeMiniSwatch meta={item} size="md" />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center justify-between gap-1">
                    <span className="truncate text-[11px] font-semibold">{item.name}</span>
                    {active && (
                      <Check
                        className="h-3 w-3 shrink-0"
                        style={{ color: "var(--accent-primary)" }}
                      />
                    )}
                  </div>
                  <span
                    className="block truncate text-[9.5px]"
                    style={{ color: "var(--text-muted)" }}
                  >
                    {item.badge}
                  </span>
                </div>
              </button>
            );
          })}
        </div>

        <PeekColorPickerControl />
      </div>
    );
  }

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        aria-label={`Switch visual theme (current: ${themeMeta.name})`}
        title={`Theme: ${themeMeta.name} — Click to switch`}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((v) => !v);
        }}
        className={cn(
          "dmd-interactive-btn inline-flex h-7 items-center gap-1.5 px-2 text-[10.5px] font-medium focus:outline-none",
          className,
        )}
        style={{
          backgroundColor: "var(--bg-surface)",
          color: "var(--text-primary)",
          borderColor: "var(--border-color)",
          borderWidth: "var(--border-width)",
          borderStyle: "solid",
          borderRadius: "var(--radius-md)",
          boxShadow: "var(--shadow-button)",
        }}
      >
        <ThemeMiniSwatch meta={themeMeta} size="sm" />
        <Palette className="h-3 w-3 shrink-0" style={{ color: "var(--text-secondary)" }} />
      </button>

      {open &&
        coords &&
        typeof document !== "undefined" &&
        createPortal(
          <div
            ref={menuRef}
            id={menuId}
            role="dialog"
            aria-label="Select Visual Theme"
            onClick={(e) => e.stopPropagation()}
            style={{
              position: "fixed",
              top: coords.top,
              bottom: coords.bottom,
              right: coords.right,
              width: 292,
              backgroundColor: "var(--bg-popover)",
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
            className="z-[9999] max-h-[85vh] space-y-2.5 overflow-y-auto p-2.5 transition-all duration-200"
          >
            <div className="flex items-center justify-between px-1">
              <span className="text-[11px] font-bold tracking-tight">Visual Theme Engine</span>
              <span
                className="rounded px-1.5 py-0.5 text-[9.5px] font-semibold"
                style={{
                  backgroundColor: "var(--bg-surface)",
                  color: "var(--text-secondary)",
                  borderColor: "var(--border-color)",
                  borderWidth: "var(--border-width)",
                  borderStyle: "solid",
                  borderRadius: "var(--radius-sm)",
                }}
              >
                {themeMeta.name}
              </span>
            </div>

            <div className="space-y-1.5" role="radiogroup" aria-label="Visual themes">
              {themes.map((item) => {
                const active = item.id === theme;
                return (
                  <button
                    key={item.id}
                    type="button"
                    role="radio"
                    aria-checked={active}
                    onClick={() => {
                      setTheme(item.id);
                    }}
                    className="dmd-interactive-btn flex w-full cursor-pointer items-center gap-2.5 p-2 text-left transition-all"
                    style={{
                      backgroundColor: active ? "var(--bg-surface-hover)" : "var(--bg-surface)",
                      borderColor: active ? "var(--accent-primary)" : "var(--border-color)",
                      borderWidth: "var(--border-width)",
                      borderStyle: "solid",
                      borderRadius: "var(--radius-md)",
                      boxShadow: active ? "var(--shadow-card)" : "var(--shadow-button)",
                      color: "var(--text-primary)",
                    }}
                  >
                    <ThemeMiniSwatch meta={item} size="md" />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center justify-between gap-1">
                        <span className="truncate text-[11.5px] font-semibold">{item.name}</span>
                        <span
                          className="shrink-0 rounded px-1.5 py-0.2 text-[9px] font-medium"
                          style={{
                            backgroundColor: active
                              ? "var(--accent-primary)"
                              : "var(--bg-surface-hover)",
                            color: active ? "var(--accent-foreground)" : "var(--text-secondary)",
                            borderRadius: "var(--radius-sm)",
                          }}
                        >
                          {item.badge}
                        </span>
                      </div>
                      <p
                        className="mt-0.5 truncate text-[10px]"
                        style={{ color: "var(--text-muted)" }}
                      >
                        {item.description}
                      </p>
                    </div>
                  </button>
                );
              })}
            </div>

            <PeekColorPickerControl />
          </div>,
          document.body,
        )}
    </>
  );
}
