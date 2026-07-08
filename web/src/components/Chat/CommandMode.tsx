import { useState } from "react";
import { Plus, Target, Repeat } from "lucide-react";

interface CommandModeProps {
  value: "goal" | "loop" | null;
  onChange: (v: "goal" | "loop" | null) => void;
  disabled: boolean;
  connected: boolean;
}

export function CommandMode({
  value,
  onChange,
  disabled,
  connected,
}: CommandModeProps) {
  const [open, setOpen] = useState(false);

  return (
    <div className="relative flex items-center">
      <button
        onClick={() => setOpen((v) => !v)}
        disabled={disabled || !connected}
        className="inline-flex items-center gap-1 cursor-pointer text-xs text-[var(--text-muted)] hover:bg-[var(--surface-hover)] rounded-md px-1.5 py-0.5"
      >
        {value === null && <Plus className="w-4 h-4" />}
        {value === "goal" && <><Target className="w-3.5 h-3.5" /> Goal</>}
        {value === "loop" && <><Repeat className="w-3.5 h-3.5" /> Loop</>}
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-[2199]" onClick={() => setOpen(false)} />
          <div className="absolute bottom-full left-0 mb-1 w-28 rounded-md bg-[var(--surface-elevated)] border border-[var(--border-base)] shadow-lg z-[2200] overflow-hidden">
            <button
              onClick={() => { onChange("goal"); setOpen(false); }}
              className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-left text-[var(--text-primary)] hover:bg-[var(--surface-hover)] transition-colors"
            >
              <Target className="w-3 h-3" /> Goal
            </button>
            <button
              onClick={() => { onChange("loop"); setOpen(false); }}
              className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-left text-[var(--text-primary)] hover:bg-[var(--surface-hover)] transition-colors"
            >
              <Repeat className="w-3 h-3" /> Loop
            </button>
          </div>
        </>
      )}
    </div>
  );
}
