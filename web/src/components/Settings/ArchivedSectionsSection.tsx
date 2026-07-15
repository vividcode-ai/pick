import { MessageSquare, ArchiveRestore, Trash2, Folder } from "lucide-react";
import { useState, useMemo } from "react";
import type { SessionEntry } from "../../stores/sessions";
import { getEnvType } from "../../stores/env";
import { ConfirmDialog } from "./ConfirmDialog";

interface ArchivedSessionsSectionProps {
  sessions: SessionEntry[];
  onUnarchive: (id: string) => void;
  onDelete: (id: string) => void;
}

function formatRelativeTime(ts: number): string {
  const diff = Date.now() - ts;
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  const months = Math.floor(days / 30);
  return `${months}mo ago`;
}

export function ArchivedSessionsSection({ sessions, onUnarchive, onDelete }: ArchivedSessionsSectionProps) {
  const [confirmId, setConfirmId] = useState<string | null>(null);
  const [openProject, setOpenProject] = useState<string | null>(null);

  const grouped = useMemo(() => {
    const groups = new Map<string, SessionEntry[]>();
    for (const s of sessions) {
      const key = s.cwd || "__default__";
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(s);
    }
    return Array.from(groups.entries())
      .map(([cwd, items]) => ({
        cwd,
        name: cwd === "__default__" ? "Other" : cwd.split(/[\\/]/).pop() || cwd,
        sessions: items,
      }))
      .sort((a, b) => {
        if (a.cwd === "__default__") return 1;
        if (b.cwd === "__default__") return -1;
        return a.name.localeCompare(b.name);
      });
  }, [sessions]);

  if (sessions.length === 0) {
    return (
      <div className="text-sm text-[var(--text-muted)] text-center py-12">
        No archived sessions
      </div>
    );
  }

  const isTauri = getEnvType() === "tauri";

  return (
    <div className="border border-[var(--border-base)] rounded-md overflow-hidden p-3 space-y-1">
      {isTauri ? (
        /* ── Tauri: project-grouped accordion ── */
        grouped.map((group) => {
          const isOpen = openProject === group.cwd;
          return (
            <div key={group.cwd}>
              <button
                onClick={() => setOpenProject((prev) => (prev === group.cwd ? null : group.cwd))}
                className={`flex items-center gap-1.5 w-full px-3 py-1.5 text-xs font-semibold rounded-lg transition-colors ${
                  isOpen
                    ? "bg-blue-600/15 text-blue-400"
                    : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)] hover:text-[var(--text-primary)]"
                }`}
              >
                <Folder className="w-4 h-4 shrink-0" />
                <span className="truncate">{group.name}</span>
                <span className="text-[10px] ml-auto opacity-60">{group.sessions.length}</span>
              </button>
              {isOpen && (
                <div className="ml-3 mt-0.5 space-y-0.5 border-l border-[var(--border-base)] pl-2">
                  {group.sessions.map((session) => (
                    <div key={session.id} className="flex items-center gap-3 px-3 py-2 rounded-md hover:bg-[var(--surface-hover)] transition-colors text-xs">
                      <MessageSquare className="w-3.5 h-3.5 text-[var(--text-muted)] flex-shrink-0" />
                      <div className="flex-1 min-w-0">
                        <div className="text-[var(--text-primary)] truncate">{session.title}</div>
                        <div className="text-[10px] text-[var(--text-muted)]">Created {formatRelativeTime(session.createdAt)}</div>
                      </div>
                      <div className="flex items-center gap-0.5">
                        <button onClick={() => onUnarchive(session.id)} className="p-1 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors" title="Unarchive"><ArchiveRestore className="w-3.5 h-3.5" /></button>
                        <button onClick={() => setConfirmId(session.id)} className="p-1 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-red-400 transition-colors" title="Delete"><Trash2 className="w-3.5 h-3.5" /></button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          );
        })
      ) : (
        /* ── Web: flat list (original) ── */
        sessions.map((session, i) => (
          <div key={session.id}>
            <div className="flex items-center gap-3 px-4 py-3 rounded-md hover:bg-[var(--surface-hover)] transition-colors">
              <MessageSquare className="w-4 h-4 text-[var(--text-muted)] flex-shrink-0" />
              <div className="flex-1 min-w-0">
                <div className="text-sm text-[var(--text-primary)] truncate">{session.title}</div>
                <div className="text-[11px] text-[var(--text-muted)]">Created {formatRelativeTime(session.createdAt)}</div>
              </div>
              <div className="flex items-center gap-1">
                <button onClick={() => onUnarchive(session.id)} className="p-1.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors" title="Unarchive"><ArchiveRestore className="w-4 h-4" /></button>
                <button onClick={() => setConfirmId(session.id)} className="p-1.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-red-400 transition-colors" title="Delete"><Trash2 className="w-4 h-4" /></button>
              </div>
            </div>
            {i < sessions.length - 1 && <div className="h-px bg-[var(--border-base)] mx-4" />}
          </div>
        ))
      )}

      {confirmId && (
        <ConfirmDialog
          message="Are you sure you want to permanently delete this session? This action cannot be undone."
          onConfirm={() => {
            onDelete(confirmId);
            setConfirmId(null);
          }}
          onCancel={() => setConfirmId(null)}
        />
      )}
    </div>
  );
}
