import { useState, useEffect, useCallback } from "react";
import { Check, Trash2 } from "lucide-react";

interface AgentSectionProps {
  serverUrl: string;
}

interface PromptEditorState {
  value: string;
  saving: boolean;
  deleting: boolean;
  msg: string;
}

function makeState(initial = ""): PromptEditorState {
  return { value: initial, saving: false, deleting: false, msg: "" };
}

type Field = "system_prompt" | "append_prompt";
type Scope = "project" | "global";

export function AgentSection({ serverUrl }: AgentSectionProps) {
  const [ps, setPs] = useState<PromptEditorState>(makeState);
  const [pa, setPa] = useState<PromptEditorState>(makeState);
  const [gs, setGs] = useState<PromptEditorState>(makeState);
  const [ga, setGa] = useState<PromptEditorState>(makeState);

  const setVal = useCallback(
    (scope: Scope, field: Field, val: string) => {
      const setter = scope === "project"
        ? field === "system_prompt" ? setPs : setPa
        : field === "system_prompt" ? setGs : setGa;
      setter((s) => ({ ...s, value: val, msg: "" }));
    },
    [],
  );

  const setBusy = useCallback(
    (scope: Scope, field: Field, key: "saving" | "deleting", busy: boolean) => {
      const setter = scope === "project"
        ? field === "system_prompt" ? setPs : setPa
        : field === "system_prompt" ? setGs : setGa;
      setter((s) => ({ ...s, [key]: busy }));
    },
    [],
  );

  const setMsg = useCallback(
    (scope: Scope, field: Field, msg: string) => {
      const setter = scope === "project"
        ? field === "system_prompt" ? setPs : setPa
        : field === "system_prompt" ? setGs : setGa;
      setter((s) => ({ ...s, msg }));
    },
    [],
  );

  useEffect(() => {
    fetch(`${serverUrl}/agent/prompts`)
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        if (data) {
          setPs((s) => ({ ...s, value: data.system_prompt?.project ?? "" }));
          setPa((s) => ({ ...s, value: data.append_prompt?.project ?? "" }));
          setGs((s) => ({ ...s, value: data.system_prompt?.global ?? "" }));
          setGa((s) => ({ ...s, value: data.append_prompt?.global ?? "" }));
        }
      })
      .catch(() => {});
  }, [serverUrl]);

  const handleSave = async (scope: Scope, field: Field, value: string) => {
    if (!value.trim()) return;
    setBusy(scope, field, "saving", true);
    setMsg(scope, field, "");
    try {
      const res = await fetch(`${serverUrl}/agent/prompts`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          [field]: value,
          scope,
        }),
      });
      setMsg(scope, field, res.ok ? "Saved" : "Failed to save");
    } catch {
      setMsg(scope, field, "Failed to save");
    }
    setBusy(scope, field, "saving", false);
  };

  const handleDelete = async (scope: Scope, field: Field) => {
    setBusy(scope, field, "deleting", true);
    setMsg(scope, field, "");
    try {
      const res = await fetch(`${serverUrl}/agent/prompts`, {
        method: "DELETE",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ scope, target: field }),
      });
      if (res.ok) {
        setVal(scope, field, "");
        setMsg(scope, field, "Deleted");
      } else {
        setMsg(scope, field, "Failed to delete");
      }
    } catch {
      setMsg(scope, field, "Failed to delete");
    }
    setBusy(scope, field, "deleting", false);
  };

  const renderEditor = (
    scope: Scope,
    field: Field,
    state: PromptEditorState,
    label: string,
    path: string,
    description: string,
    placeholder: string,
  ) => (
    <div className="flex flex-col gap-2">
      <label className="text-sm font-medium">
        {label} <span className="text-xs text-neutral-500">({path})</span>
      </label>
      <p className="text-xs text-neutral-400">{description}</p>
      <textarea
        className="w-full h-32 p-3 rounded-md bg-[var(--surface-secondary)] border border-[var(--border-base)] text-sm font-mono resize-y focus:outline-none focus:border-[var(--accent)]"
        value={state.value}
        onChange={(e) => setVal(scope, field, e.target.value)}
        placeholder={placeholder}
      />
      <div className="flex items-center gap-2">
        <button
          onClick={() => handleSave(scope, field, state.value)}
          disabled={state.saving || state.deleting || !state.value.trim()}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-[var(--accent)] text-white text-xs font-medium hover:opacity-90 disabled:opacity-40 transition-opacity"
        >
          <Check className="w-3.5 h-3.5" />
          {state.saving ? "Saving..." : "Save"}
        </button>
        <button
          onClick={() => handleDelete(scope, field)}
          disabled={state.saving || state.deleting}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-red-600 text-white text-xs font-medium hover:opacity-90 disabled:opacity-40 transition-opacity"
        >
          <Trash2 className="w-3.5 h-3.5" />
          {state.deleting ? "Deleting..." : "Delete"}
        </button>
        {state.msg && (
          <span className="text-xs text-neutral-400">{state.msg}</span>
        )}
      </div>
    </div>
  );

  return (
    <div className="flex flex-col gap-8">
      <div className="flex flex-col gap-4">
        <h3 className="text-sm font-semibold text-[var(--text-primary)] border-b border-[var(--border-base)] pb-1">
          Project Level
        </h3>
        {renderEditor(
          "project", "system_prompt", ps,
          "System Prompt", ".pick/SYSTEM.md",
          "Replaces the default system prompt for this project. Falls back to global when absent.",
          "Enter system prompt...",
        )}
        {renderEditor(
          "project", "append_prompt", pa,
          "Append Prompt", ".pick/APPEND_SYSTEM.md",
          "Appended to the end of the system prompt for this project.",
          "Enter append prompt...",
        )}
      </div>

      <div className="flex flex-col gap-4">
        <h3 className="text-sm font-semibold text-[var(--text-primary)] border-b border-[var(--border-base)] pb-1">
          System / Global Level
        </h3>
        {renderEditor(
          "global", "system_prompt", gs,
          "System Prompt", "~/.pick/agent/SYSTEM.md",
          "Replaces the default system prompt globally. Project-level takes priority.",
          "Enter system prompt...",
        )}
        {renderEditor(
          "global", "append_prompt", ga,
          "Append Prompt", "~/.pick/agent/APPEND_SYSTEM.md",
          "Appended to the end of the system prompt globally. Merged with project-level append.",
          "Enter append prompt...",
        )}
      </div>
    </div>
  );
}
