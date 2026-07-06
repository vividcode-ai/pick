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

function PromptEditor({
  scope,
  field,
  state,
  label,
  path,
  description,
  placeholder,
  onVal,
  onSave,
  onDelete,
}: {
  scope: Scope;
  field: Field;
  state: PromptEditorState;
  label: string;
  path: string;
  description: string;
  placeholder: string;
  onVal: (scope: Scope, field: Field, val: string) => void;
  onSave: (scope: Scope, field: Field, value: string) => void;
  onDelete: (scope: Scope, field: Field) => void;
}) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <label className="text-sm font-medium">
          {label} <span className="text-xs text-neutral-500">({path})</span>
        </label>
        <button
          onClick={() => onDelete(scope, field)}
          disabled={state.saving || state.deleting}
          className="flex items-center gap-1 px-2 py-1 rounded text-xs border border-[var(--border-base)] text-[var(--text-secondary)] hover:text-red-500 hover:border-red-500 disabled:opacity-40 transition-colors"
        >
          <Trash2 className="w-3 h-3" />
          {state.deleting ? "Deleting..." : "Delete"}
        </button>
      </div>
      <p className="text-xs text-neutral-400">{description}</p>
      <textarea
        className="w-full h-32 p-3 rounded-md bg-[var(--surface-secondary)] border border-[var(--border-base)] text-sm font-mono resize-y focus:outline-none focus:border-[var(--accent-primary)]"
        value={state.value}
        onChange={(e) => onVal(scope, field, e.target.value)}
        placeholder={placeholder}
      />
      <div className="flex items-center justify-end gap-2">
        <button
          onClick={() => onSave(scope, field, state.value)}
          disabled={state.saving || state.deleting || !state.value.trim()}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-md border border-[var(--accent-primary)] bg-[var(--accent-primary)] text-white text-xs font-medium hover:opacity-90 disabled:opacity-40 transition-opacity"
        >
          <Check className="w-3.5 h-3.5" />
          {state.saving ? "Saving..." : "Save"}
        </button>
        {state.msg && (
          <span className="text-xs text-neutral-400">{state.msg}</span>
        )}
      </div>
    </div>
  );
}

export function AgentSection({ serverUrl }: AgentSectionProps) {
  const [ps, setPs] = useState<PromptEditorState>(makeState);
  const [pa, setPa] = useState<PromptEditorState>(makeState);
  const [gs, setGs] = useState<PromptEditorState>(makeState);
  const [ga, setGa] = useState<PromptEditorState>(makeState);

  const setVal = useCallback(
    (scope: Scope, field: Field, val: string) => {
      const setter =
        scope === "project"
          ? field === "system_prompt"
            ? setPs
            : setPa
          : field === "system_prompt"
            ? setGs
            : setGa;
      setter((s) => ({ ...s, value: val, msg: "" }));
    },
    [],
  );

  const setBusy = useCallback(
    (
      scope: Scope,
      field: Field,
      key: "saving" | "deleting",
      busy: boolean,
    ) => {
      const setter =
        scope === "project"
          ? field === "system_prompt"
            ? setPs
            : setPa
          : field === "system_prompt"
            ? setGs
            : setGa;
      setter((s) => ({ ...s, [key]: busy }));
    },
    [],
  );

  const setMsg = useCallback(
    (scope: Scope, field: Field, msg: string) => {
      const setter =
        scope === "project"
          ? field === "system_prompt"
            ? setPs
            : setPa
          : field === "system_prompt"
            ? setGs
            : setGa;
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
        body: JSON.stringify({ [field]: value, scope }),
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

  return (
    <div className="flex flex-col gap-6">
      <div className="border border-[var(--border-base)] rounded-md p-4 flex flex-col gap-4">
        <h3 className="text-sm font-semibold text-[var(--text-primary)]">
          Project Level
        </h3>
        <PromptEditor
          scope="project"
          field="system_prompt"
          state={ps}
          label="System Prompt"
          path=".pick/SYSTEM.md"
          description="Replaces the default system prompt for this project. Falls back to global when absent."
          placeholder="Enter system prompt..."
          onVal={setVal}
          onSave={handleSave}
          onDelete={handleDelete}
        />
        <hr className="border-[var(--border-base)]" />
        <PromptEditor
          scope="project"
          field="append_prompt"
          state={pa}
          label="Append Prompt"
          path=".pick/APPEND_SYSTEM.md"
          description="Appended to the end of the system prompt for this project."
          placeholder="Enter append prompt..."
          onVal={setVal}
          onSave={handleSave}
          onDelete={handleDelete}
        />
      </div>

      <div className="border border-[var(--border-base)] rounded-md p-4 flex flex-col gap-4">
        <h3 className="text-sm font-semibold text-[var(--text-primary)]">
          System / Global Level
        </h3>
        <PromptEditor
          scope="global"
          field="system_prompt"
          state={gs}
          label="System Prompt"
          path="~/.pick/agent/SYSTEM.md"
          description="Replaces the default system prompt globally. Project-level takes priority."
          placeholder="Enter system prompt..."
          onVal={setVal}
          onSave={handleSave}
          onDelete={handleDelete}
        />
        <hr className="border-[var(--border-base)]" />
        <PromptEditor
          scope="global"
          field="append_prompt"
          state={ga}
          label="Append Prompt"
          path="~/.pick/agent/APPEND_SYSTEM.md"
          description="Appended to the end of the system prompt globally. Merged with project-level append."
          placeholder="Enter append prompt..."
          onVal={setVal}
          onSave={handleSave}
          onDelete={handleDelete}
        />
      </div>
    </div>
  );
}
