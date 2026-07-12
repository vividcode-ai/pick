import { LoopJobCard } from "./LoopJobCard";
import type { LoopJobResponse } from "../../types/events";

interface LoopPanelProps {
  jobs: LoopJobResponse[];
  baseUrl: string;
  sessionId: string | null;
}

export function LoopPanel({ jobs, baseUrl, sessionId }: LoopPanelProps) {
  if (jobs.length === 0) return null;

  const handlePause = async (id: string) => {
    if (!sessionId) return;
    await fetch(`${baseUrl}/sessions/${sessionId}/loops/${id}/pause`, { method: "POST" });
  };

  const handleResume = async (id: string) => {
    if (!sessionId) return;
    await fetch(`${baseUrl}/sessions/${sessionId}/loops/${id}/resume`, { method: "POST" });
  };

  const handleDelete = async (id: string) => {
    if (!sessionId) return;
    await fetch(`${baseUrl}/sessions/${sessionId}/loops/${id}`, { method: "DELETE" });
  };

  const handleTrigger = async (id: string) => {
    if (!sessionId) return;
    await fetch(`${baseUrl}/sessions/${sessionId}/loops/${id}/trigger`, { method: "POST" });
  };

  const activeCount = jobs.filter((j) => j.status === "idle" || j.status === "running").length;
  const title = jobs.length === 1 ? jobs[0].action : `🔄 Loops (${activeCount}/${jobs.length})`;

  return (
    <div className="w-full flex flex-col gap-1.5">
      {jobs.length === 1 ? null : (
        <div className="text-xs text-[var(--text-muted)] font-medium px-1">
          {title}
        </div>
      )}
      {jobs.map((job) => (
        <LoopJobCard
          key={job.id}
          job={job}
          onPause={handlePause}
          onResume={handleResume}
          onDelete={handleDelete}
          onTrigger={handleTrigger}
          baseUrl={baseUrl}
          sessionId={sessionId}
        />
      ))}
    </div>
  );
}
