import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Folder, X } from "lucide-react";
import { useProjectsState, fetchProjects } from "../../stores/projects";

interface ProjectModalProps {
  open: boolean;
  onClose: () => void;
  baseUrl: string;
  isTauri: boolean;
  onProjectSwitch: (path: string) => void;
}

export function ProjectModal({
  open,
  onClose,
  baseUrl,
  isTauri,
  onProjectSwitch,
}: ProjectModalProps) {
  const { projects, currentCwd, loading } = useProjectsState();
  const [switching, setSwitching] = useState<string | null>(null);

  useEffect(() => {
    if (open && baseUrl) {
      fetchProjects(baseUrl);
    }
  }, [open, baseUrl]);

  const handlePickAndSwitch = useCallback(async () => {
    if (!isTauri) return;
    try {
      const path = await invoke<string | null>("pick_directory");
      if (path) {
        await onProjectSwitch(path);
      }
    } catch {
      // pick cancelled
    }
  }, [isTauri, onProjectSwitch]);

  const handleProjectClick = useCallback(
    async (path: string) => {
      setSwitching(path);
      try {
        await onProjectSwitch(path);
      } catch {
        // ignore
      }
      setSwitching(null);
    },
    [onProjectSwitch]
  );

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg bg-[var(--surface-elevated)] border border-[var(--border-base)] rounded-xl shadow-2xl max-h-[80vh] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--border-base)] shrink-0">
          <div className="flex items-center gap-2">
            <Folder className="w-5 h-5 text-[var(--accent-primary)]" />
            <h2 className="text-base font-semibold text-[var(--text-primary)]">Project Management</h2>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-4">
          {/* Open Project button — shown at top when in Tauri */}
          {isTauri && (
            <button
              onClick={handlePickAndSwitch}
              className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-blue-600 hover:bg-blue-700 text-white rounded-lg text-sm font-medium transition-colors"
            >
              <Folder className="w-4 h-4" />
              Open Project
            </button>
          )}

          {/* Project list */}
          <div>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-[var(--text-muted)] mb-2">
              History Projects
            </h3>
            {loading ? (
              <div className="text-sm text-[var(--text-muted)] text-center py-4">Loading...</div>
            ) : projects.length === 0 ? (
              <div className="text-sm text-[var(--text-muted)] text-center py-4">
                No projects yet.
              </div>
            ) : (
              <div className="space-y-0.5">
                {projects.map((proj) => {
                  const isActive = currentCwd && currentCwd === proj.path;
                  return (
                    <div
                      key={proj.path}
                      role="button"
                      tabIndex={0}
                      onClick={() => !isActive && handleProjectClick(proj.path)}
                      onKeyDown={(e) => { if (e.key === "Enter" && !isActive) handleProjectClick(proj.path); }}
                      className={`flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm cursor-pointer transition-colors ${
                        isActive
                          ? "bg-blue-600/10 border border-blue-500/30 cursor-default"
                          : "hover:bg-[var(--surface-hover)] border border-transparent"
                      } ${switching === proj.path ? "opacity-50 pointer-events-none" : ""}`}
                    >
                      <Folder className="w-4 h-4 shrink-0 text-[var(--text-muted)]" />
                      <div className="flex-1 min-w-0">
                        <div className="font-medium text-[var(--text-primary)] truncate">
                          {proj.name}
                        </div>
                        <div className="text-xs text-[var(--text-muted)] truncate">
                          {proj.path}
                        </div>
                      </div>
                      {isActive && (
                        <span className="text-xs text-blue-500 shrink-0 font-medium">Active</span>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
