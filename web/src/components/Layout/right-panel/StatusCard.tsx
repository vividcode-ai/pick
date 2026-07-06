import { useState, useCallback } from "react";
import { GitBranch, FolderOpen, FileCode, RotateCw } from "lucide-react";
import type { GitInfo } from "../../../types/events";
import { CommitModal } from "./CommitModal";

interface StatusCardProps {
  gitInfo: GitInfo | null;
  sessionId: string | null;
  onCommitRequest: (message: string) => void;
}

const statusLabel: Record<string, string> = {
  M: "Modified",
  A: "Added",
  D: "Deleted",
  R: "Renamed",
  "??": "Untracked",
  "!!": "Ignored",
};

const statusColor: Record<string, string> = {
  M: "text-yellow-400 bg-yellow-500/10",
  A: "text-green-400 bg-green-500/10",
  D: "text-red-400 bg-red-500/10",
  R: "text-blue-400 bg-blue-500/10",
  "??": "text-neutral-400 bg-neutral-500/10",
};

export function StatusCard({ gitInfo, sessionId, onCommitRequest }: StatusCardProps) {
  const [commitOpen, setCommitOpen] = useState(false);
  const [showAll, setShowAll] = useState(false);

  const handleCommit = useCallback((message: string) => {
    onCommitRequest(message);
  }, [onCommitRequest]);

  if (!sessionId) return null;

  const changes = gitInfo?.changes ?? [];
  const changeCount = changes.length;
  const displayChanges = showAll ? changes : changes.slice(0, 5);

  return (
    <div className="rounded-xl border border-[var(--border-base)] bg-[var(--surface-secondary)] shadow-sm overflow-hidden">
      <div className="px-3 py-2.5 border-b border-[var(--border-base)]">
        <h3 className="text-xs font-semibold text-[var(--text-primary)] uppercase tracking-wider">
          Status
        </h3>
      </div>
      <div className="p-3 space-y-3">
        {/* Workspace directory */}
        <div className="flex items-start gap-2">
          <FolderOpen className="w-3.5 h-3.5 mt-0.5 text-[var(--text-muted)] flex-shrink-0" />
          <div className="min-w-0 flex-1">
            <div className="text-[10px] text-[var(--text-muted)] font-medium">Working Directory</div>
            <div className="text-xs text-[var(--text-primary)] truncate" title={gitInfo?.cwd ?? ""}>
              {gitInfo?.cwd ?? "—"}
            </div>
          </div>
        </div>

        {/* Git branch */}
        <div className="flex items-start gap-2">
          <GitBranch className="w-3.5 h-3.5 mt-0.5 text-[var(--text-muted)] flex-shrink-0" />
          <div className="min-w-0 flex-1">
            <div className="text-[10px] text-[var(--text-muted)] font-medium">Branch</div>
            <div className="text-xs text-[var(--accent-primary)] font-mono">
              {gitInfo?.branch ?? "—"}
            </div>
          </div>
        </div>

        {/* Git changes */}
        <div className="flex items-start gap-2">
          <FileCode className="w-3.5 h-3.5 mt-0.5 text-[var(--text-muted)] flex-shrink-0" />
          <div className="min-w-0 flex-1">
            <div className="text-[10px] text-[var(--text-muted)] font-medium">
              Modified Files
              {changeCount > 0 && (
                <span className="ml-1.5 text-[var(--accent-primary)]">
                  ({changeCount})
                </span>
              )}
            </div>
            {changeCount > 0 ? (
              <div className="mt-1.5 space-y-1">
                {displayChanges.map((change, i) => {
                  const st = change.status.trim();
                  const label = statusLabel[st] ?? st;
                  const color = statusColor[st] ?? "text-neutral-400 bg-neutral-500/10";
                  return (
                    <div key={i} className="flex items-center gap-1.5 text-xs">
                      <span className={`text-[10px] px-1 py-0.5 rounded font-mono font-medium ${color}`}>
                        {label}
                      </span>
                      <span className="text-[var(--text-secondary)] truncate flex-1" title={change.path}>
                        {change.path}
                      </span>
                    </div>
                  );
                })}
                {changeCount > 5 && !showAll && (
                  <button
                    onClick={() => setShowAll(true)}
                    className="text-xs text-[var(--accent-primary)] hover:underline mt-1"
                  >
                    Show all {changeCount}
                  </button>
                )}
                {showAll && changeCount > 5 && (
                  <button
                    onClick={() => setShowAll(false)}
                    className="text-xs text-[var(--text-muted)] hover:underline mt-1"
                  >
                    Collapse
                  </button>
                )}
              </div>
            ) : (
              <div className="text-xs text-[var(--text-muted)] mt-0.5">
                No uncommitted changes
              </div>
            )}
          </div>
        </div>

        {/* Commit button */}
        {changeCount > 0 && (
          <button
            onClick={() => setCommitOpen(true)}
            className="w-full flex items-center justify-center gap-1.5 px-3 py-2 text-xs font-medium rounded-lg bg-[var(--accent-primary)] text-white hover:bg-[var(--accent-hover)] transition-colors"
          >
            <RotateCw className="w-3.5 h-3.5" />
            Commit
          </button>
        )}
      </div>

      <CommitModal
        open={commitOpen}
        onClose={() => setCommitOpen(false)}
        onCommit={handleCommit}
      />
    </div>
  );
}
