import { MessageSquare, Trash2, Pencil } from "lucide-react";
import { useState } from "react";
import type { SessionEntry } from "../../stores/sessions";

interface SessionItemProps {
  session: SessionEntry;
  isActive: boolean;
  onSelect: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onDelete: (id: string) => void;
}

export function SessionItem({ session, isActive, onSelect, onRename, onDelete }: SessionItemProps) {
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
      className={`group flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer transition-colors text-sm ${
        isActive
          ? "bg-neutral-800 text-neutral-100"
          : "text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
      }`}
      onClick={() => onSelect(session.id)}
    >
      <MessageSquare className="w-3.5 h-3.5 flex-shrink-0" />
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
          onClick={(e) => { e.stopPropagation(); onDelete(session.id); }}
          className="p-1 rounded hover:bg-neutral-700 text-neutral-500 hover:text-red-400"
          title="Delete"
        >
          <Trash2 className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
}
