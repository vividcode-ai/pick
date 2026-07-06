import { useState, useEffect } from "react";
import { Check } from "lucide-react";

interface AgentSectionProps {
  serverUrl: string;
}

export function AgentSection({ serverUrl }: AgentSectionProps) {
  const [systemPrompt, setSystemPrompt] = useState("");
  const [appendPrompt, setAppendPrompt] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");

  useEffect(() => {
    fetch(`${serverUrl}/agent/prompts`)
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        if (data) {
          setSystemPrompt(data.system_prompt ?? "");
          setAppendPrompt(data.append_prompt ?? "");
        }
      })
      .catch(() => {});
  }, [serverUrl]);

  const handleSave = async () => {
    setSaving(true);
    setMessage("");
    try {
      const res = await fetch(`${serverUrl}/agent/prompts`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          system_prompt: systemPrompt,
          append_prompt: appendPrompt,
        }),
      });
      if (res.ok) {
        setMessage("Saved successfully");
      } else {
        setMessage("Failed to save");
      }
    } catch {
      setMessage("Failed to save");
    }
    setSaving(false);
  };

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-2">
        <label className="text-sm font-medium">
          System Prompt <span className="text-xs text-neutral-500">(&rarr; SYSTEM.md)</span>
        </label>
        <p className="text-xs text-neutral-400">
          Replaces the default system prompt entirely. Leave empty to use the
          default.
        </p>
        <textarea
          className="w-full h-40 p-3 rounded-md bg-[var(--surface-secondary)] border border-[var(--border-base)] text-sm font-mono resize-y focus:outline-none focus:border-[var(--accent)]"
          value={systemPrompt}
          onChange={(e) => setSystemPrompt(e.target.value)}
          placeholder="Enter system prompt..."
        />
      </div>

      <div className="flex flex-col gap-2">
        <label className="text-sm font-medium">
          Append Prompt{" "}
          <span className="text-xs text-neutral-500">
            (&rarr; APPEND_SYSTEM.md)
          </span>
        </label>
        <p className="text-xs text-neutral-400">
          Appended to the end of the system prompt. Useful for additional
          instructions.
        </p>
        <textarea
          className="w-full h-32 p-3 rounded-md bg-[var(--surface-secondary)] border border-[var(--border-base)] text-sm font-mono resize-y focus:outline-none focus:border-[var(--accent)]"
          value={appendPrompt}
          onChange={(e) => setAppendPrompt(e.target.value)}
          placeholder="Enter append prompt..."
        />
      </div>

      <div className="flex items-center gap-3">
        <button
          onClick={handleSave}
          disabled={saving}
          className="flex items-center gap-2 px-4 py-2 rounded-md bg-[var(--accent)] text-white text-sm font-medium hover:opacity-90 disabled:opacity-50 transition-opacity"
        >
          <Check className="w-4 h-4" />
          {saving ? "Saving..." : "Save"}
        </button>
        {message && (
          <span className="text-xs text-neutral-400">{message}</span>
        )}
      </div>
    </div>
  );
}
