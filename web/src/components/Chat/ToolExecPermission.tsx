import { getAppSettings, setSetting, subscribeAppSettings } from "../../stores/appSettings";
import { useState, useEffect } from "react";

export function ToolExecPermission() {
  const [mode, setMode] = useState<"prompt" | "auto_approve">(
    getAppSettings().tool_execution_permission,
  );

  useEffect(() => {
    const unsub = subscribeAppSettings(() => {
      setMode(getAppSettings().tool_execution_permission);
    });
    return () => { unsub(); };
  }, []);

  const toggle = () => {
    const next = mode === "prompt" ? "auto_approve" : "prompt";
    setSetting("tool_execution_permission", next);
  };

  return (
    <div className="relative flex items-center">
      <button
        onClick={toggle}
        className="inline-flex items-center gap-1 cursor-pointer text-xs text-[var(--text-muted)] hover:bg-[var(--surface-hover)] rounded-md px-1.5 py-0.5"
        title={mode === "prompt" ? "需要询问授权" : "完全自动执行"}
      >
        {mode === "prompt" ? "Prompt" : "Auto"}
      </button>
    </div>
  );
}
