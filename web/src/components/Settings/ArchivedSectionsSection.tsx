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
      <div className="text-sm text-neutral-500 text-center py-12">
        No archived sessions
      </div>
    );
  }

  return (
    <div className="border border-neutral-700 rounded-md overflow-hidden divide-y divide-neutral-700">
      {sessions.map((session) => (
        <div
          key={session.id}
          className="flex items-center gap-3 px-3 py-2.5 hover:bg-neutral-800/50 transition-colors"
        >
          <MessageSquare className="w-4 h-4 text-neutral-500 flex-shrink-0" />
          <div className="flex-1 min-w-0">
            <div className="text-sm text-neutral-200 truncate">{session.title}</div>
            <div className="text-[11px] text-neutral-500">
              Created {formatRelativeTime(session.createdAt)}
            </div>
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={() => onUnarchive(session.id)}
              className="p-1.5 rounded hover:bg-neutral-700 text-neutral-500 hover:text-neutral-200 transition-colors"
              title="Unarchive"
            >
              <ArchiveRestore className="w-4 h-4" />
            </button>
            <button
              onClick={() => setConfirmId(session.id)}
              className="p-1.5 rounded hover:bg-neutral-700 text-neutral-500 hover:text-red-400 transition-colors"
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
