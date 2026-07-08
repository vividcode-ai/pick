import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";

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
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    document.documentElement.setAttribute("data-theme", prefersDark ? "dark" : "light");
  } else {
    document.documentElement.setAttribute("data-theme", mode);
  }
}

function resolveDarkTheme(mode: ThemeMode): boolean {
  if (mode === "dark") return true;
  if (mode === "light") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [themeMode, setThemeModeState] = useState<ThemeMode>("system");
  const [isDark, setIsDark] = useState(
    () => document.documentElement.getAttribute("data-theme") === "dark",
  );

  const isFirstMount = useRef(true);

  const applyResolvedTheme = useCallback((mode: ThemeMode) => {
    const dark = resolveDarkTheme(mode);
    applyThemeMode(mode);
    setIsDark(dark);
  }, []);

  useEffect(() => {
    if (isFirstMount.current) {
      isFirstMount.current = false;
      return;
    }
    applyResolvedTheme(themeMode);
  }, [themeMode, applyResolvedTheme]);

  useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      if (themeMode === "system") {
        applyResolvedTheme("system");
      }
    };
    mediaQuery.addEventListener("change", handler);
    return () => mediaQuery.removeEventListener("change", handler);
  }, [themeMode, applyResolvedTheme]);

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
