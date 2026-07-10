import { useState } from "react";
import { Plus } from "lucide-react";
import { LoopJobCard } from "./LoopJobCard";
import type { LoopJobResponse } from "../../types/events";

interface LoopPanelProps {
  jobs: LoopJobResponse[];
  baseUrl: string;
  sessionId: string | null;
}

export function LoopPanel({ jobs, baseUrl, sessionId }: LoopPanelProps) {
  const [showCreate, setShowCreate] = useState(false);
  const [newPrompt, setNewPrompt] = useState("");
  const [newInterval, setNewInterval] = useState("5m");

  if (jobs.length === 0 && !showCreate) return null;

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

  const handleCreate = async () => {
    if (!sessionId || !newPrompt.trim()) return;
    const intervalMs = parseInterval(newInterval);
    await fetch(`${baseUrl}/sessions/${sessionId}/loops`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ action: newPrompt.trim(), interval_ms: intervalMs, immediate: true }),
    });
    setNewPrompt("");
    setShowCreate(false);
  };

  const activeCount = jobs.filter((j) => j.status === "idle" || j.status === "running").length;

  return (
    <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[40%] mx-auto w-full mb-2">
      <div className="flex items-center justify-between mb-1.5 px-1">
        <span className="text-xs text-[var(--text-muted)] font-medium">
          🔄 Loops ({activeCount}/{jobs.length})
        </span>
        <button
          onClick={() => setShowCreate(!showCreate)}
          className="flex items-center gap-1 text-xs text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
        >
          <Plus className="w-3 h-3" />
          {showCreate ? "Cancel" : "Add"}
        </button>
      </div>

      {showCreate && (
        <div className="flex items-center gap-2 px-3 py-2 mb-2 rounded-lg border border-[var(--border-base)] bg-[var(--surface-base)]">
          <input
            value={newInterval}
            onChange={(e) => setNewInterval(e.target.value)}
            className="w-16 bg-transparent text-xs text-[var(--text-primary)] outline-none border-b border-[var(--border-base)] text-center"
            placeholder="5m"
          />
          <input
            value={newPrompt}
            onChange={(e) => setNewPrompt(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") handleCreate(); if (e.key === "Escape") setShowCreate(false); }}
            className="flex-1 bg-transparent text-sm text-[var(--text-primary)] outline-none"
            placeholder="Loop prompt..."
            autoFocus
          />
          <button
            onClick={handleCreate}
            disabled={!newPrompt.trim()}
            className="px-2 py-1 text-xs rounded bg-[var(--surface-hover)] text-[var(--text-primary)] disabled:opacity-40"
          >
            Create
          </button>
        </div>
      )}

      <div className="flex flex-col gap-1.5">
        {jobs.map((job) => (
          <LoopJobCard
            key={job.id}
            job={job}
            onPause={handlePause}
            onResume={handleResume}
            onDelete={handleDelete}
            onTrigger={handleTrigger}
          />
        ))}
      </div>
    </div>
  );
}

function parseInterval(s: string): number {
  const match = s.match(/^(\d+)(s|m|h)?$/);
  if (!match) return 300_000;
  const num = parseInt(match[1]);
  const unit = match[2] || "m";
  switch (unit) {
    case "s": return num * 1000;
    case "h": return num * 3600_000;
    default: return num * 60_000;
  }
}
