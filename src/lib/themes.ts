import { createContext, useContext } from "react";

export type ThemeId =
  | "minimalism"
  | "glassmorphism"
  | "brutalism"
  | "neumorphism"
  | "oled-dark"
  | "skeuomorphism"
  | "dark-aurora"
  | "dark-blur";

export interface ThemeMeta {
  id: ThemeId;
  name: string;
  badge: string;
  description: string;
  swatch: {
    canvas: string;
    surface: string;
    border: string;
    borderWidth: string;
    accent: string;
    secondaryAccent?: string;
    text: string;
    shadow: string;
    radius: string;
    clipPath?: string;
  };
}

export const THEME_STORAGE_KEY = "app-theme";
export const PEEK_COLOR_STORAGE_KEY = "app-peek-color";

export const PEEK_COLOR_PRESETS = [
  { label: "Sky Cyan", value: "#38bdf8" },
  { label: "Indigo", value: "#6366f1" },
  { label: "Emerald", value: "#10b981" },
  { label: "Amber", value: "#f59e0b" },
  { label: "Rose", value: "#f43f5e" },
  { label: "Violet", value: "#a855f7" },
  { label: "Neon Lime", value: "#a3e635" },
] as const;

export const THEMES: readonly ThemeMeta[] = [
  {
    id: "minimalism",
    name: "Minimalism",
    badge: "Clean Whitespace",
    description: "Pure #FAFAFA canvas, 1px hairline border, monochrome #18181B accent.",
    swatch: {
      canvas: "#FAFAFA",
      surface: "#FFFFFF",
      border: "#E5E7EB",
      borderWidth: "1px",
      accent: "#18181B",
      text: "#111827",
      shadow: "0 1px 2px rgba(0,0,0,0.05)",
      radius: "5px",
    },
  },
  {
    id: "glassmorphism",
    name: "Glassmorphism",
    badge: "Frosted Blur 16px",
    description: "Multi-color pastel mesh gradient with translucent frosted glass cards.",
    swatch: {
      canvas: "linear-gradient(135deg, #E0C3FC 0%, #8EC5FC 50%, #E0E7FF 100%)",
      surface: "rgba(255, 255, 255, 0.42)",
      border: "rgba(255, 255, 255, 0.7)",
      borderWidth: "1px",
      accent: "#4F46E5",
      text: "#0F172A",
      shadow: "0 4px 12px rgba(31, 38, 135, 0.16)",
      radius: "7px",
    },
  },
  {
    id: "brutalism",
    name: "Neo-Brutalism",
    badge: "2.5px + Hard Shadow",
    description: "Warm #FFFDF5 canvas, #FFDE59 & #A3E635 accents, 2.5px solid black borders.",
    swatch: {
      canvas: "#FFFDF5",
      surface: "#FFDE59",
      border: "#000000",
      borderWidth: "2px",
      accent: "#A3E635",
      secondaryAccent: "#000000",
      text: "#000000",
      shadow: "2px 2px 0px #000000",
      radius: "3px",
    },
  },
  {
    id: "neumorphism",
    name: "Neumorphism",
    badge: "Soft UI Extruded",
    description: "Monochromatic #E0E5EC surface with dual extruded & inset soft shadows.",
    swatch: {
      canvas: "#E0E5EC",
      surface: "#E0E5EC",
      border: "transparent",
      borderWidth: "0px",
      accent: "#4F46E5",
      text: "#2D3748",
      shadow: "3px 3px 6px #B8BEC7, -3px -3px 6px #FFFFFF",
      radius: "8px",
    },
  },
  {
    id: "skeuomorphism",
    name: "Skeuomorphism",
    badge: "Tactile Gloss Bevel",
    description: "Physical #DCD8D0 surface, top specular highlight, glossy buttons & recessed inputs.",
    swatch: {
      canvas: "#DCD8D0",
      surface: "linear-gradient(180deg, #FFFFFF 0%, #ECE8E0 100%)",
      border: "#B0ABA0",
      borderWidth: "1px",
      accent: "#2563EB",
      secondaryAccent: "#57534E",
      text: "#1C1917",
      shadow: "0 2px 5px rgba(0,0,0,0.18), inset 0 1px 0 rgba(255,255,255,0.95)",
      radius: "6px",
    },
  },
  {
    id: "oled-dark",
    name: "OLED Minimalist",
    badge: "Pure #000000",
    description: "True pitch-black OLED canvas, #0A0A0A surface, hairline borders & #38BDF8 accent.",
    swatch: {
      canvas: "#000000",
      surface: "#0A0A0A",
      border: "rgba(255, 255, 255, 0.22)",
      borderWidth: "1px",
      accent: "#38BDF8",
      text: "#EDEDED",
      shadow: "0 0 10px rgba(255,255,255,0.06)",
      radius: "6px",
    },
  },
  {
    id: "dark-aurora",
    name: "Dark Aurora",
    badge: "Cosmic Blur 14px",
    description: "Cosmic #050814 night canvas with emerald, violet & cyan aurora glow.",
    swatch: {
      canvas:
        "radial-gradient(circle at 20% 20%, rgba(16,185,129,0.45), transparent 55%), radial-gradient(circle at 80% 25%, rgba(139,92,246,0.5), transparent 55%), #050814",
      surface: "rgba(12, 17, 32, 0.72)",
      border: "rgba(139, 92, 246, 0.45)",
      borderWidth: "1px",
      accent: "#2DD4BF",
      secondaryAccent: "#A78BFA",
      text: "#F8FAFC",
      shadow: "0 4px 12px rgba(6, 182, 212, 0.2)",
      radius: "6px",
    },
  },
  {
    id: "dark-blur",
    name: "Dark Blur",
    badge: "Smoked Acrylic 24px",
    description: "Dark smoked frosted glass (blur 24px saturate 160%) with #60A5FA ice accent.",
    swatch: {
      canvas:
        "radial-gradient(circle at 25% 25%, rgba(148,163,184,0.22), transparent 50%), radial-gradient(circle at 75% 75%, rgba(71,85,105,0.28), transparent 55%), #090B10",
      surface: "rgba(24, 27, 36, 0.58)",
      border: "rgba(255, 255, 255, 0.24)",
      borderWidth: "1px",
      accent: "#60A5FA",
      secondaryAccent: "#E5E7EB",
      text: "#F3F4F6",
      shadow: "inset 0 1px 0 rgba(255,255,255,0.22), 0 4px 12px rgba(0,0,0,0.55)",
      radius: "7px",
    },
  },
] as const;

const VALID_THEME_IDS = new Set<ThemeId>(THEMES.map((t) => t.id));
let memoryThemeFallback: ThemeId | null = null;
let memoryPeekColorFallback: string | null = null;

export function isValidThemeId(value: unknown): value is ThemeId {
  return typeof value === "string" && VALID_THEME_IDS.has(value as ThemeId);
}

export function isValidHexColor(value: unknown): value is string {
  return typeof value === "string" && /^#[0-9a-fA-F]{6}$/.test(value.trim());
}

export function readStoredPeekColor(): string | null {
  try {
    if (typeof window !== "undefined" && typeof window.localStorage?.getItem === "function") {
      const val = window.localStorage.getItem(PEEK_COLOR_STORAGE_KEY);
      if (isValidHexColor(val)) return val.toLowerCase();
    }
  } catch {
    // Ignore storage access errors
  }
  return memoryPeekColorFallback;
}

export function applyPeekColorToDom(color: string | null): void {
  const normalized = isValidHexColor(color) ? color.toLowerCase() : null;
  memoryPeekColorFallback = normalized;
  if (typeof document !== "undefined") {
    if (normalized) {
      document.documentElement.style.setProperty("--peek-custom-color", normalized);
    } else {
      document.documentElement.style.removeProperty("--peek-custom-color");
    }
  }
  try {
    if (typeof window !== "undefined" && typeof window.localStorage?.setItem === "function") {
      if (normalized) {
        window.localStorage.setItem(PEEK_COLOR_STORAGE_KEY, normalized);
      } else {
        window.localStorage.removeItem(PEEK_COLOR_STORAGE_KEY);
      }
    }
  } catch {
    // Ignore storage errors
  }
}

export function readStoredTheme(): ThemeId | null {
  try {
    if (typeof window !== "undefined" && typeof window.localStorage?.getItem === "function") {
      const val = window.localStorage.getItem(THEME_STORAGE_KEY);
      if (isValidThemeId(val)) return val;
    }
  } catch {
    // Ignore storage access errors
  }
  return memoryThemeFallback;
}

export function clearStoredTheme(): void {
  memoryThemeFallback = null;
  memoryPeekColorFallback = null;
  if (typeof document !== "undefined") {
    document.documentElement.style.removeProperty("--peek-custom-color");
  }
  try {
    if (typeof window !== "undefined" && typeof window.localStorage?.removeItem === "function") {
      window.localStorage.removeItem(THEME_STORAGE_KEY);
      window.localStorage.removeItem(PEEK_COLOR_STORAGE_KEY);
    }
  } catch {
    // Ignore storage access errors
  }
}

export function getInitialTheme(): ThemeId {
  if (typeof document !== "undefined") {
    const domAttr = document.documentElement.getAttribute("data-theme");
    if (isValidThemeId(domAttr)) return domAttr;
  }
  return readStoredTheme() ?? "oled-dark";
}

export function applyThemeToDom(theme: ThemeId): void {
  memoryThemeFallback = theme;
  if (typeof document !== "undefined") {
    document.documentElement.setAttribute("data-theme", theme);
  }
  try {
    if (typeof window !== "undefined" && typeof window.localStorage?.setItem === "function") {
      window.localStorage.setItem(THEME_STORAGE_KEY, theme);
    }
  } catch {
    // Ignore storage errors
  }
}

export interface ThemeContextValue {
  theme: ThemeId;
  themeMeta: ThemeMeta;
  themes: readonly ThemeMeta[];
  setTheme: (next: ThemeId) => void;
  peekColor: string | null;
  setPeekColor: (color: string | null) => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) {
    const fallbackTheme = getInitialTheme();
    const themeMeta =
      THEMES.find((t) => t.id === fallbackTheme) ??
      THEMES.find((t) => t.id === "oled-dark") ??
      THEMES[0];
    return {
      theme: fallbackTheme,
      themeMeta,
      themes: THEMES,
      setTheme: applyThemeToDom,
      peekColor: readStoredPeekColor(),
      setPeekColor: applyPeekColorToDom,
    };
  }
  return ctx;
}
