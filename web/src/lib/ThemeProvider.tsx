import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";

export type ThemeMode = "system" | "light" | "dark";

interface ThemeContextValue {
  isDark: boolean;
  themeMode: ThemeMode;
  setThemeMode: (mode: ThemeMode) => void;
  cycleThemeMode: () => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

function applyThemeMode(mode: ThemeMode) {
  if (mode === "system") {
    document.documentElement.removeAttribute("data-theme");
    return;
  }
  document.documentElement.setAttribute("data-theme", mode);
}

function resolveDarkTheme(mode: ThemeMode): boolean {
  if (mode === "dark") return true;
  if (mode === "light") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [themeMode, setThemeModeState] = useState<ThemeMode>("system");
  const [isDark, setIsDark] = useState(true);

  const applyResolvedTheme = useCallback((mode: ThemeMode) => {
    const dark = resolveDarkTheme(mode);
    applyThemeMode(mode);
    setIsDark(dark);
  }, []);

  useEffect(() => {
    applyResolvedTheme(themeMode);
  }, [themeMode, applyResolvedTheme]);

  useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      if (themeMode === "system") {
        setIsDark(mediaQuery.matches);
        applyThemeMode("system");
      }
    };
    mediaQuery.addEventListener("change", handler);
    return () => mediaQuery.removeEventListener("change", handler);
  }, [themeMode]);

  const setThemeMode = useCallback((mode: ThemeMode) => {
    setThemeModeState(mode);
    applyResolvedTheme(mode);
  }, [applyResolvedTheme]);

  const cycleThemeMode = useCallback(() => {
    setThemeModeState((prev) => {
      const next: ThemeMode = prev === "system" ? "light" : prev === "light" ? "dark" : "system";
      applyResolvedTheme(next);
      return next;
    });
  }, [applyResolvedTheme]);

  return (
    <ThemeContext.Provider value={{ isDark, themeMode, setThemeMode, cycleThemeMode }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) throw new Error("useTheme must be used within ThemeProvider");
  return context;
}
