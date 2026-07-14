import { useState, useSyncExternalStore } from "react";
import {
  getAppSettings,
  subscribeAppSettings,
  setSetting,
} from "../../stores/appSettings";

function useAppSettings() {
  return useSyncExternalStore(subscribeAppSettings, getAppSettings, getAppSettings);
}

export function PermissionSection() {
  const settings = useAppSettings();
  const [text, setText] = useState(
    (settings.network_allowed_domains ?? []).join("\n")
  );
  const [saved, setSaved] = useState(false);

  const handleSave = () => {
    const list = text
      .split("\n")
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    setSetting("network_allowed_domains", list);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const handleReset = () => {
    setText("");
    setSetting("network_allowed_domains", []);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  return (
    <div className="flex flex-col gap-4">
      <div>
        <p className="text-sm text-[var(--text-primary)] font-medium mb-1">
          Allowed Network Domains
        </p>
        <p className="text-xs text-[var(--text-secondary)] mb-3">
          Domains not in this list will prompt for authorization when the agent
          tries to fetch them. Blocked domains (private IPs, localhost) are
          always denied regardless of this list.
        </p>
        <textarea
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            setSaved(false);
          }}
          placeholder={`*.baidu.com\ntophub.today\napi.example.com`}
          rows={8}
          className="w-full px-3 py-2 rounded-md bg-[var(--surface-base)] border border-[var(--border-base)] text-sm text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent)] font-mono resize-y"
        />
        <p className="text-xs text-[var(--text-tertiary)] mt-1">
          One domain pattern per line. Use <code className="text-[var(--accent)]">*.example.com</code> for wildcards.
        </p>
      </div>

      <div className="flex items-center gap-3">
        <button
          onClick={handleSave}
          className="px-4 py-2 rounded-md bg-[var(--accent-primary)] text-white text-sm font-medium hover:opacity-90 transition-opacity"
        >
          Save
        </button>
        <button
          onClick={handleReset}
          className="px-4 py-2 rounded-md bg-[var(--surface-hover)] text-[var(--text-primary)] text-sm border border-[var(--border-base)] hover:bg-[var(--surface-button)] hover:border-[var(--text-muted)] transition-colors"
        >
          Reset
        </button>
        {saved && (
          <span className="text-sm text-green-500">Saved</span>
        )}
      </div>
    </div>
  );
}
