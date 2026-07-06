import { useState, useEffect } from "react";
import { Check } from "lucide-react";

interface AgentSectionProps {
  serverUrl: string;
}

type Scope = "project" | "global";

export function AgentSection({ serverUrl }: AgentSectionProps) {
  const [scope, setScope] = useState<Scope>("project");
  const [systemPrompt, setSystemPrompt] = useState("");
  const [appendPrompt, setAppendPrompt] = useState("");
  const [systemPromptGlobal, setSystemPromptGlobal] = useState("");
  const [appendPromptGlobal, setAppendPromptGlobal] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");

  useEffect(() => {
    fetch(`${serverUrl}/agent/prompts`)
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        if (data) {
          setSystemPrompt(data.system_prompt?.project ?? "");
          setSystemPromptGlobal(data.system_prompt?.global ?? "");
          setAppendPrompt(data.append_prompt?.project ?? "");
          setAppendPromptGlobal(data.append_prompt?.global ?? "");
        }
      })
      .catch(() => {});
  }, [serverUrl]);

  const currentSystem = scope === "project" ? systemPrompt : systemPromptGlobal;
  const currentAppend = scope === "project" ? appendPrompt : appendPromptGlobal;

  const handleSave = async () => {
    setSaving(true);
    setMessage("");
    try {
      const res = await fetch(`${serverUrl}/agent/prompts`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          system_prompt: currentSystem,
          append_prompt: currentAppend,
          scope,
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
      <div className="flex items-center gap-1 bg-[var(--surface-secondary)] rounded-md p-1 w-fit border border-[var(--border-base)]">
        <button
          onClick={() => setScope("project")}
          className={`px-3 py-1.5 text-xs rounded font-medium transition-colors ${
            scope === "project"
              ? "bg-[var(--accent)] text-white"
              : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
          }`}
        >
          Project
        </button>
        <button
          onClick={() => setScope("global")}
          className={`px-3 py-1.5 text-xs rounded font-medium transition-colors ${
            scope === "global"
              ? "bg-[var(--accent)] text-white"
              : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
          }`}
        >
          Global
        </button>
      </div>

      <div className="flex flex-col gap-2">
        <label className="text-sm font-medium">
          System Prompt{" "}
          <span className="text-xs text-neutral-500">
            ({scope === "project" ? ".pick/SYSTEM.md" : "~/.pick/agent/SYSTEM.md"})
          </span>
        </label>
        <p className="text-xs text-neutral-400">
          {scope === "project"
            ? "Replaces the default system prompt for this project. Leave empty to fall back to global."
            : "Replaces the default system prompt globally. Project-level takes priority over global."}
        </p>
        <textarea
          className="w-full h-40 p-3 rounded-md bg-[var(--surface-secondary)] border border-[var(--border-base)] text-sm font-mono resize-y focus:outline-none focus:border-[var(--accent)]"
          value={currentSystem}
          onChange={(e) =>
            scope === "project"
              ? setSystemPrompt(e.target.value)
              : setSystemPromptGlobal(e.target.value)
          }
          placeholder="Enter system prompt..."
        />
      </div>

      <div className="flex flex-col gap-2">
        <label className="text-sm font-medium">
          Append Prompt{" "}
          <span className="text-xs text-neutral-500">
            ({scope === "project" ? ".pick/APPEND_SYSTEM.md" : "~/.pick/agent/APPEND_SYSTEM.md"})
          </span>
        </label>
        <p className="text-xs text-neutral-400">
          {scope === "project"
            ? "Appended to the end of the system prompt for this project."
            : "Appended to the end of the system prompt globally. Both project and global append prompts are merged."}
        </p>
        <textarea
          className="w-full h-32 p-3 rounded-md bg-[var(--surface-secondary)] border border-[var(--border-base)] text-sm font-mono resize-y focus:outline-none focus:border-[var(--accent)]"
          value={currentAppend}
          onChange={(e) =>
            scope === "project"
              ? setAppendPrompt(e.target.value)
              : setAppendPromptGlobal(e.target.value)
          }
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
