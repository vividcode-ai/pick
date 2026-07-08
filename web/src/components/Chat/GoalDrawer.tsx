import { useState, useEffect, useRef } from "react";
import { Target, Pencil, Clock, Trash2, Play, Pause } from "lucide-react";

interface GoalInfo {
  objective: string;
  startTime: number;
}

interface GoalDrawerProps {
  goal: GoalInfo | null;
  onEdit: (newObjective: string) => void;
  onPause: () => void;
  onDelete: () => void;
}

function formatElapsed(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
}

export function GoalDrawer({ goal, onEdit, onPause, onDelete }: GoalDrawerProps) {
  const [editing, setEditing] = useState(false);
  const [paused, setPaused] = useState(false);
  const [editText, setEditText] = useState("");
  const [elapsed, setElapsed] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!goal) return;
    setEditText(goal.objective);
    const tick = () => setElapsed(Date.now() - goal.startTime);
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [goal]);

  useEffect(() => {
    if (editing && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [editing]);

  if (!goal) return null;

  const handleSaveEdit = () => {
    const trimmed = editText.trim();
    if (!trimmed) return;
    onEdit(trimmed);
    setEditing(false);
  };

  const handleCancelEdit = () => {
    setEditText(goal.objective);
    setEditing(false);
  };

  const handlePause = () => {
    setPaused((v) => !v);
    onPause();
  };

  return (
    <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[40%] mx-auto w-full mb-2">
      <div className="flex items-center justify-between gap-3 px-3 py-2 rounded-lg border border-[var(--border-base)] bg-[var(--surface-base)]">
        {/* Left: icon + objective + timer */}
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <Target className="w-4 h-4 shrink-0 text-[var(--text-muted)]" />
          {editing ? (
            <input
              ref={inputRef}
              value={editText}
              onChange={(e) => setEditText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSaveEdit();
                if (e.key === "Escape") handleCancelEdit();
              }}
              onBlur={handleSaveEdit}
              className="flex-1 min-w-0 bg-transparent text-sm text-[var(--text-primary)] outline-none border-b border-[var(--border-base)]"
            />
          ) : (
            <span className="text-sm text-[var(--text-primary)] truncate">{goal.objective}</span>
          )}
          <span className="flex items-center gap-1 text-xs text-[var(--text-muted)] shrink-0">
            <Clock className="w-3 h-3" />
            {formatElapsed(elapsed)}
          </span>
        </div>

        {/* Right: action buttons */}
        <div className="flex items-center gap-1 shrink-0">
          <button
            onClick={() => { if (editing) handleCancelEdit(); else { setEditText(goal.objective); setEditing(true); } }}
            className="p-1 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
            title={editing ? "Cancel" : "Edit"}
          >
            <Pencil className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={handlePause}
            className="p-1 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
            title={paused ? "Resume" : "Pause"}
          >
            {paused ? <Play className="w-3.5 h-3.5" /> : <Pause className="w-3.5 h-3.5" />}
          </button>
          <button
            onClick={onDelete}
            className="p-1 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-red-400 transition-colors"
            title="Delete goal"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>
    </div>
  );
}
