import { useState, useRef, useEffect } from "react";
import { X, Shield } from "lucide-react";

interface ApiKeyDialogProps {
  provider: string;
  baseUrl: string;
  onClose: () => void;
  onSuccess: () => void;
}

export function ApiKeyDialog({ provider, baseUrl, onClose, onSuccess }: ApiKeyDialogProps) {
  const [key, setKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setTimeout(() => inputRef.current?.focus(), 80);
  }, []);

  const handleSave = async () => {
    const trimmed = key.trim();
    if (!trimmed) {
      setError("API key cannot be empty");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const res = await fetch(`${baseUrl}/providers/${encodeURIComponent(provider)}/key`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ key: trimmed }),
      });
      if (!res.ok) {
        const text = await res.text();
        setError(text || "Failed to save API key");
        return;
      }
      onSuccess();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Network error");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={onClose}
    >
      <div
        className="bg-[var(--surface-base)] border border-[var(--border-base)] rounded-xl shadow-xl w-[380px] flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-base)]">
          <div className="flex items-center gap-2">
            <Shield className="w-4 h-4 text-[var(--accent-primary)]" />
            <h2 className="text-sm font-semibold text-[var(--text-primary)]">API Key</h2>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--surface-hover)] transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body */}
        <div className="px-4 py-4 space-y-3">
          <p className="text-xs text-[var(--text-muted)]">
            Enter an API key for <span className="font-semibold text-[var(--text-primary)]">{provider}</span>.
            This will be saved to your local auth storage.
          </p>
          <div className="relative">
            <input
              ref={inputRef}
              type={showKey ? "text" : "password"}
              value={key}
              onChange={(e) => { setKey(e.target.value); setError(null); }}
              onKeyDown={(e) => { if (e.key === "Enter") handleSave(); }}
              placeholder="sk-..."
              className="w-full pr-8 pl-3 py-2 text-xs border border-[var(--border-base)] rounded-md bg-[var(--surface-base)] text-[var(--text-primary)] outline-none focus:border-[var(--accent-primary)] focus:shadow-[0_0_0_1px_var(--accent-primary)] placeholder-[var(--text-muted)]"
              spellCheck={false}
              autoComplete="off"
            />
            <button
              onClick={() => setShowKey((v) => !v)}
              className="absolute right-2 top-1/2 -translate-y-1/2 px-1 py-0.5 text-[10px] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors uppercase tracking-wider"
              tabIndex={-1}
            >
              {showKey ? "Hide" : "Show"}
            </button>
          </div>
          {error && (
            <p className="text-xs text-red-500">{error}</p>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-[var(--border-base)]">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-xs rounded-md bg-[var(--surface-button)] text-[var(--text-primary)] hover:opacity-80 transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={saving}
            className="px-3 py-1.5 text-xs rounded-md bg-[var(--accent-primary)] text-white hover:opacity-80 disabled:opacity-50 transition-colors"
          >
            {saving ? "Saving..." : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
