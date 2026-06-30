import { useTheme, type ThemeMode } from "../../lib/ThemeProvider";
import { Laptop, Moon, Sun } from "lucide-react";

const themeOptions: { value: ThemeMode; icon: typeof Sun; label: string }[] = [
  { value: "system", icon: Laptop, label: "System" },
  { value: "light", icon: Sun, label: "Light" },
  { value: "dark", icon: Moon, label: "Dark" },
];

export function AppearanceSection() {
  const { themeMode, setThemeMode } = useTheme();

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-sm font-semibold text-neutral-100">Theme</h3>
        <p className="text-xs text-neutral-500 mt-1">Choose your preferred appearance</p>
      </div>
      <div className="grid grid-cols-3 gap-3">
        {themeOptions.map((opt) => {
          const Icon = opt.icon;
          const selected = themeMode === opt.value;
          return (
            <button
              key={opt.value}
              onClick={() => setThemeMode(opt.value)}
              className={`flex flex-col items-center gap-2 p-4 rounded-lg border text-sm transition-colors ${
                selected
                  ? "border-blue-500 bg-blue-500/10 text-blue-400"
                  : "border-neutral-700 bg-neutral-800 text-neutral-400 hover:bg-neutral-750"
              }`}
            >
              <Icon className="w-5 h-5" />
              <span>{opt.label}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
