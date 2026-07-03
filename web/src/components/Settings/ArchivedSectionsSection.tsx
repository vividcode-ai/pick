import { MessageSquare, ArchiveRestore, Trash2 } from "lucide-react";
import { useState } from "react";
import type { SessionEntry } from "../../stores/sessions";
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

  if (sessions.length === 0) {
    return (
      <div className="text-sm text-[var(--text-muted)] text-center py-12">
        No archived sessions
      </div>
    );
  }

  return (
    <div className="border border-[var(--border-base)] rounded-md p-2 space-y-1">
      {sessions.map((session) => (
        <div
          key={session.id}
          className="flex items-center gap-3 px-3 py-2.5 rounded-md hover:bg-[var(--surface-hover)] transition-colors"
        >
          <MessageSquare className="w-4 h-4 text-[var(--text-muted)] flex-shrink-0" />
          <div className="flex-1 min-w-0">
            <div className="text-sm text-[var(--text-primary)] truncate">{session.title}</div>
            <div className="text-[11px] text-[var(--text-muted)]">
              Created {formatRelativeTime(session.createdAt)}
            </div>
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={() => onUnarchive(session.id)}
              className="p-1.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
              title="Unarchive"
            >
              <ArchiveRestore className="w-4 h-4" />
            </button>
            <button
              onClick={() => setConfirmId(session.id)}
              className="p-1.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-red-400 transition-colors"
              title="Delete"
            >
              <Trash2 className="w-4 h-4" />
            </button>
          </div>
        </div>
      ))}

      {confirmId && (
        <ConfirmDialog
          message="确定要永久删除此会话吗？此操作不可撤销。"
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
