import { MessageSquare, Archive, Pencil } from "lucide-react";
import { useState } from "react";
import type { SessionEntry } from "../../stores/sessions";

interface SessionItemProps {
  session: SessionEntry;
  isActive: boolean;
  streaming: boolean;
  onSelect: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onArchive: (id: string) => void;
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

export function SessionItem({ session, isActive, streaming, onSelect, onRename, onArchive }: SessionItemProps) {
  const [editing, setEditing] = useState(false);
  const [editTitle, setEditTitle] = useState(session.title);

  const handleRename = () => {
    if (editTitle.trim() && editTitle !== session.title) {
      onRename(session.id, editTitle.trim());
    }
    setEditing(false);
  };

  return (
    <div
      className={`group flex flex-col gap-0 px-3 py-2 rounded-md cursor-pointer transition-colors text-sm ${
        isActive
          ? "bg-neutral-800 text-neutral-100"
          : "text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
      }`}
      onClick={() => onSelect(session.id)}
    >
      <div className="flex items-center gap-2">
        {streaming ? (
          <span className="w-3.5 h-3.5 flex items-center justify-center flex-shrink-0">
            <span className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
          </span>
        ) : (
          <MessageSquare className="w-3.5 h-3.5 flex-shrink-0" />
        )}
        {editing ? (
          <input
            type="text"
            value={editTitle}
            onChange={(e) => setEditTitle(e.target.value)}
            onBlur={handleRename}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleRename();
              if (e.key === "Escape") setEditing(false);
            }}
            className="flex-1 bg-transparent text-sm outline-none border-b border-neutral-500"
            autoFocus
            onClick={(e) => e.stopPropagation()}
          />
        ) : (
          <span className="flex-1 truncate">{session.title}</span>
        )}
        <div className="hidden group-hover:flex items-center gap-0.5">
          <button
            onClick={(e) => { e.stopPropagation(); setEditing(true); setEditTitle(session.title); }}
            className="p-1 rounded hover:bg-neutral-700 text-neutral-500 hover:text-neutral-300"
            title="Rename"
          >
            <Pencil className="w-3 h-3" />
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); onArchive(session.id); }}
            className="p-1 rounded hover:bg-neutral-700 text-neutral-500 hover:text-neutral-300"
            title="Archive"
          >
            <Archive className="w-3 h-3" />
          </button>
        </div>
      </div>
      <span className="text-[11px] text-neutral-500 pl-[22px]">{formatRelativeTime(session.createdAt)}</span>
    </div>
  );
}
