interface SelectSettingOption {
  value: string;
  label: string;
}

interface SelectSettingProps {
  label: string;
  description?: string;
  options: SelectSettingOption[];
  value: string;
  onChange: (value: string) => void;
}

export function SelectSetting({ label, description, options, value, onChange }: SelectSettingProps) {
  return (
    <div className="settings-row">
      <div className="flex-1 min-w-0">
        <div className="settings-row-label">{label}</div>
        {description && <div className="settings-row-description">{description}</div>}
      </div>
      <div className="flex gap-1.5 flex-shrink-0">
        {options.map((opt) => {
          const selected = value === opt.value;
          return (
            <button
              key={opt.value}
              onClick={() => onChange(opt.value)}
              className={`px-2.5 py-1 rounded text-xs font-medium transition-colors ${
                selected
                  ? "bg-blue-500/20 text-blue-400 border border-blue-500/30"
                  : "bg-neutral-800 text-neutral-400 border border-neutral-700 hover:bg-neutral-700"
              }`}
            >
              {opt.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}
