import { Play, Pause, Trash2, Repeat } from "lucide-react";
import type { LoopJobResponse } from "../../types/events";

interface LoopJobCardProps {
  job: LoopJobResponse;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onDelete: (id: string) => void;
  onTrigger: (id: string) => void;
}

const STATUS_ICONS: Record<string, string> = {
  idle: "🔄",
  running: "▶️",
  paused: "⏸",
  done: "✅",
  failed: "❌",
};

function formatNextDue(ms: number): string {
  if (ms <= 0) return "now";
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ${secs % 60}s`;
  const hours = Math.floor(mins / 60);
  return `${hours}h ${mins % 60}m`;
}

export function LoopJobCard({ job, onPause, onResume, onDelete, onTrigger }: LoopJobCardProps) {
  const statusColor =
    job.status === "running"
      ? "text-green-500"
      : job.status === "paused"
        ? "text-yellow-500"
        : job.status === "failed"
          ? "text-red-500"
          : job.status === "done"
            ? "text-blue-500"
            : "text-[var(--text-muted)]";

  return (
    <div className="flex items-center justify-between gap-3 px-3 py-2 rounded-lg border border-[var(--border-base)] bg-[var(--surface-base)]">
      <div className="flex items-center gap-2 min-w-0 flex-1">
        <span className={`text-sm ${statusColor}`}>
          {STATUS_ICONS[job.status] || "🔄"}
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-sm text-[var(--text-primary)] font-medium truncate">
              {job.name}
            </span>
            <span className="text-xs text-[var(--text-muted)] bg-[var(--surface-hover)] px-1.5 py-0.5 rounded">
              {job.kind}
            </span>
          </div>
          <div className="flex items-center gap-3 text-xs text-[var(--text-muted)] mt-0.5">
            <span>runs {job.run_count}{job.max_runs != null ? `/${job.max_runs}` : ""}</span>
            {job.status === "idle" && (
              <span>next {formatNextDue(job.next_due_ms)}</span>
            )}
            {job.status === "paused" && <span>paused</span>}
            {job.status === "done" && <span>completed</span>}
            {job.status === "failed" && <span>failed ({job.failure_count})</span>}
            {job.action.length > 40 && <span className="truncate max-w-[200px]">{job.action}</span>}
          </div>
        </div>
      </div>

      <div className="flex items-center gap-1 shrink-0">
        {job.status === "paused" ? (
          <button
            onClick={() => onResume(job.id)}
            className="p-1 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-green-400 transition-colors"
            title="Resume"
          >
            <Play className="w-3.5 h-3.5" />
          </button>
        ) : (
          <button
            onClick={() => onPause(job.id)}
            className="p-1 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-yellow-400 transition-colors"
            title="Pause"
          >
            <Pause className="w-3.5 h-3.5" />
          </button>
        )}
        <button
          onClick={() => onTrigger(job.id)}
          className="p-1 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-blue-400 transition-colors"
          title="Trigger now"
        >
          <Repeat className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={() => onDelete(job.id)}
          className="p-1 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-red-400 transition-colors"
          title="Delete"
        >
          <Trash2 className="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  );
}
