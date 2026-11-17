import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  applyPeekColorToDom,
  applyThemeToDom,
  getInitialTheme,
  readStoredPeekColor,
  ThemeContext,
  THEMES,
  type ThemeContextValue,
  type ThemeId,
} from "../lib/themes";

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeId>(() => getInitialTheme());
  const [peekColor, setPeekColorState] = useState<string | null>(() => readStoredPeekColor());

  useEffect(() => {
    applyThemeToDom(theme);
  }, [theme]);

  useEffect(() => {
    applyPeekColorToDom(peekColor);
  }, [peekColor]);

  const setTheme = useCallback((next: ThemeId) => {
    applyThemeToDom(next);
    setThemeState(next);
  }, []);

  const setPeekColor = useCallback((color: string | null) => {
    applyPeekColorToDom(color);
    setPeekColorState(color ? color.toLowerCase() : null);
  }, []);

  const value = useMemo<ThemeContextValue>(() => {
    const themeMeta =
      THEMES.find((t) => t.id === theme) ??
      THEMES.find((t) => t.id === "oled-dark") ??
      THEMES[0];
    return {
      theme,
      themeMeta,
      themes: THEMES,
      setTheme,
      peekColor,
      setPeekColor,
    };
  }, [theme, setTheme, peekColor, setPeekColor]);

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
