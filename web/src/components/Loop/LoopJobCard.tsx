import { Play, Pause, Trash2, Repeat, CheckCircle, AlertCircle, Clock } from "lucide-react";
import { useState, useEffect, useRef } from "react";
import type { LoopJobResponse } from "../../types/events";

interface LoopJobCardProps {
  job: LoopJobResponse;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onDelete: (id: string) => void;
  onTrigger: (id: string) => void;
  baseUrl: string;
  sessionId: string | null;
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

function formatCountdown(ms: number): string {
  if (ms <= 0) return "0s";
  const totalSecs = Math.ceil(ms / 1000);
  const secs = totalSecs % 60;
  const mins = Math.floor(totalSecs / 60) % 60;
  const hours = Math.floor(totalSecs / 3600);
  if (hours > 0) return `${hours}h ${mins}m ${secs}s`;
  if (mins > 0) return `${mins}m ${secs}s`;
  return `${secs}s`;
}

export function LoopJobCard({ job, onPause, onResume, onDelete, onTrigger, baseUrl, sessionId }: LoopJobCardProps) {
  const [showGoalInput, setShowGoalInput] = useState<"complete" | "blocked" | "progress" | null>(null);
  const [goalInput, setGoalInput] = useState("");
  const [countdownMs, setCountdownMs] = useState(job.next_due_ms);
  const prevNextDueRef = useRef(job.next_due_ms);

  // Sync countdown when server pushes fresh next_due_ms
  useEffect(() => {
    if (job.next_due_ms !== prevNextDueRef.current) {
      setCountdownMs(job.next_due_ms);
      prevNextDueRef.current = job.next_due_ms;
    }
  }, [job.next_due_ms]);

  // Real-time countdown tick for idle interval-based jobs
  useEffect(() => {
    if (job.status !== "idle") return;
    const timer = setInterval(() => {
      setCountdownMs(prev => Math.max(0, prev - 1000));
    }, 1000);
    return () => clearInterval(timer);
  }, [job.status]);

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

  const isGoal = job.kind === "goal";

  const handleGoalAction = async (action: "goal-complete" | "goal-blocked" | "goal-progress") => {
    if (!sessionId) return;
    const body: Record<string, string> = {};
    if (action === "goal-complete") {
      body.summary = goalInput || "Goal completed";
      body.evidence = "Marked complete from UI";
    } else if (action === "goal-blocked") {
      body.reason = goalInput || "Blocked";
      body.needed = "User input required";
    } else if (action === "goal-progress") {
      body.summary = goalInput || "Progress made";
      body.next = "Continuing work";
    }
    await fetch(`${baseUrl}/sessions/${sessionId}/loops/${job.id}/${action}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    setGoalInput("");
    setShowGoalInput(null);
  };

  return (
    <div>
      <div className="flex items-center justify-between gap-3 px-3 py-2 rounded-lg bg-[var(--surface-base)]">
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
              {isGoal && job.goal_status && (
                <span className={`text-xs px-1.5 py-0.5 rounded ${
                  job.goal_status === "completed" ? "text-green-400 bg-green-500/10" :
                  job.goal_status === "blocked" ? "text-red-400 bg-red-500/10" :
                  "text-blue-400 bg-blue-500/10"
                }`}>
                  {job.goal_status}
                </span>
              )}
            </div>
            <div className="flex items-center gap-3 text-xs text-[var(--text-muted)] mt-0.5">
              <span>runs {job.run_count}{job.max_runs != null ? `/${job.max_runs}` : ""}</span>
              {job.status === "idle" && job.interval_ms > 0 ? (
                <span className="flex items-center gap-1 text-[var(--text-accent)]">
                  <Clock className="w-3 h-3" />
                  <span className="tabular-nums">{formatCountdown(countdownMs)}</span>
                </span>
              ) : job.status === "idle" ? (
                <span>next {formatNextDue(job.next_due_ms)}</span>
              ) : null}
              {job.status === "paused" && <span>paused</span>}
              {job.status === "done" && <span>completed</span>}
              {job.status === "failed" && <span>failed ({job.failure_count})</span>}
              {job.action && job.action.length > 40 && <span className="truncate max-w-[200px]">{job.action}</span>}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-1 shrink-0">
          {/* Goal subcommands */}
          {isGoal && job.status !== "done" && (
            <>
              <button
                onClick={() => setShowGoalInput(showGoalInput === "complete" ? null : "complete")}
                className="p-1 rounded hover:bg-[var(--surface-hover)] text-green-500 hover:text-green-400 transition-colors"
                title="Mark goal complete"
              >
                <CheckCircle className="w-3.5 h-3.5" />
              </button>
              <button
                onClick={() => setShowGoalInput(showGoalInput === "blocked" ? null : "blocked")}
                className="p-1 rounded hover:bg-[var(--surface-hover)] text-red-500 hover:text-red-400 transition-colors"
                title="Mark goal blocked"
              >
                <AlertCircle className="w-3.5 h-3.5" />
              </button>
              <button
                onClick={() => setShowGoalInput(showGoalInput === "progress" ? null : "progress")}
                className="p-1 rounded hover:bg-[var(--surface-hover)] text-blue-500 hover:text-blue-400 transition-colors"
                title="Record progress"
              >
                <span className="w-3.5 h-3.5 flex items-center justify-center text-[10px] font-bold">→</span>
              </button>
            </>
          )}
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

      {/* Goal inline input */}
      {showGoalInput && (
        <div className="flex items-center gap-2 px-3 py-2 mt-1 rounded-lg border border-[var(--border-base)] bg-[var(--surface-elevated)]/40">
          <input
            value={goalInput}
            onChange={(e) => setGoalInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                handleGoalAction(
                  showGoalInput === "complete" ? "goal-complete" :
                  showGoalInput === "blocked" ? "goal-blocked" : "goal-progress"
                );
              }
              if (e.key === "Escape") setShowGoalInput(null);
            }}
            className="flex-1 bg-transparent text-xs text-[var(--text-primary)] outline-none"
            placeholder={
              showGoalInput === "complete" ? "Completion summary..." :
              showGoalInput === "blocked" ? "Blocked reason..." :
              "Progress summary..."
            }
            autoFocus
          />
          <button
            onClick={() => handleGoalAction(
              showGoalInput === "complete" ? "goal-complete" :
              showGoalInput === "blocked" ? "goal-blocked" : "goal-progress"
            )}
            className="px-2 py-1 text-xs rounded bg-[var(--surface-hover)] text-[var(--text-primary)]"
          >
            {showGoalInput === "complete" ? "Complete" :
             showGoalInput === "blocked" ? "Block" : "Record"}
          </button>
        </div>
      )}
    </div>
  );
}
