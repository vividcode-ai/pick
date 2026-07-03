import { Laptop, Moon, Sun } from "lucide-react";
import { useTheme, type ThemeMode } from "../lib/ThemeProvider";

export function ThemeModeToggle({ className }: { className?: string }) {
  const { themeMode, cycleThemeMode } = useTheme();

  const icon: Record<ThemeMode, React.ReactNode> = {
    system: <Laptop className="w-4 h-4" />,
    light: <Sun className="w-4 h-4" />,
    dark: <Moon className="w-4 h-4" />,
  };

  return (
    <button
      type="button"
      className={className ?? "p-2 rounded-md hover:bg-[var(--surface-hover)] text-neutral-400 hover:text-[var(--text-primary)] transition-colors"}
      onClick={cycleThemeMode}
      aria-label={`Theme: ${themeMode}`}
      title={`Theme: ${themeMode}`}
    >
      {icon[themeMode]}
    </button>
  );
}
