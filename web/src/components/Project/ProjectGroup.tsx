import { useState } from "react";
import { Folder, X } from "lucide-react";
import { SessionItem } from "../Layout/SessionItem";
import type { SessionEntry } from "../../stores/sessions";

interface ProjectGroupProps {
  name: string;
  path: string;
  sessions: SessionEntry[];
  isSelected?: boolean;
  activeSessionId: string | null;
  onSelectSession: (id: string) => void;
  onRenameSession: (id: string, title: string) => void;
  onArchiveSession: (id: string) => void;
  streamingSessions?: Record<string, boolean>;
  onSelect?: () => void;
  onDelete?: (path: string) => void;
}

export function ProjectGroup({
  name,
  path,
  sessions,
  isSelected,
  activeSessionId,
  onSelectSession,
  onRenameSession,
  onArchiveSession,
  streamingSessions,
  onSelect,
  onDelete,
}: ProjectGroupProps) {
  const [hovered, setHovered] = useState(false);
  const groupName = name === "__default__" ? "Other" : name;
  const isDeletable = onDelete && path !== "__default__";

  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {/* Project header */}
      <div className="relative">
        <button
          onClick={onSelect}
          className={`flex items-center gap-1.5 w-full px-2 py-1.5 text-xs font-semibold rounded-lg transition-colors ${
            isSelected
              ? "bg-blue-600/15 text-blue-400"
              : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)] hover:text-[var(--text-primary)]"
          }`}
          title={groupName}
        >
          <Folder className="w-4 h-4 shrink-0" />
          <span className="truncate">{groupName}</span>
        </button>

        {isDeletable && hovered && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onDelete(path);
            }}
            title="Remove project from list"
            className="absolute right-1 top-1/2 -translate-y-1/2 p-0.5 rounded hover:bg-red-500/20 text-[var(--text-muted)] hover:text-red-400 transition-colors"
          >
            <X className="w-3 h-3" />
          </button>
        )}
      </div>

      {/* Session items — expand inline when selected (accordion) */}
      {isSelected && (
        <div className="ml-3 mb-1 space-y-0.5 border-l border-[var(--border-base)] pl-2">
          {sessions.length > 0 ? (
            sessions.map((session) => (
              <SessionItem
                key={session.id}
                session={session}
                isActive={session.id === activeSessionId}
                onSelect={onSelectSession}
                onRename={onRenameSession}
                onArchive={onArchiveSession}
                streaming={streamingSessions?.[session.id] ?? false}
              />
            ))
          ) : (
            <div className="text-[10px] text-[var(--text-muted)] italic py-1.5 px-1">
              No sessions in this project
            </div>
          )}
        </div>
      )}
    </div>
  );
}
