import { useSessionSearch } from "../../stores/sessions";
import { SessionSearch } from "./SessionSearch";
import { SessionItem } from "./SessionItem";
import { Plus } from "lucide-react";

interface SessionListProps {
  activeSessionId: string | null;
  onSelectSession: (id: string) => void;
  onNewSession: () => void;
  onRenameSession: (id: string, title: string) => void;
  onDeleteSession: (id: string) => void;
}

export function SessionList({
  activeSessionId,
  onSelectSession,
  onNewSession,
  onRenameSession,
  onDeleteSession,
}: SessionListProps) {
  const { query, setQuery, filtered } = useSessionSearch();

  return (
    <div className="flex flex-col min-h-0 flex-1">
      <div className="flex items-center justify-between px-4 py-3 border-b border-neutral-800">
        <span className="text-xs font-semibold uppercase text-neutral-400 tracking-wider">Sessions</span>
        <button
          onClick={onNewSession}
          className="p-1.5 rounded-md hover:bg-neutral-800 text-neutral-400 hover:text-neutral-200 transition-colors"
          title="New Session"
        >
          <Plus className="w-4 h-4" />
        </button>
      </div>
      <SessionSearch query={query} onQueryChange={setQuery} />
      <div className="flex-1 overflow-y-auto px-2 py-1 space-y-0.5">
        {filtered.length === 0 ? (
          <div className="text-xs text-neutral-500 text-center py-8">
            {query ? "No matching sessions" : "No sessions yet"}
          </div>
        ) : (
          filtered.map((session) => (
            <SessionItem
              key={session.id}
              session={session}
              isActive={session.id === activeSessionId}
              onSelect={onSelectSession}
              onRename={onRenameSession}
              onDelete={onDeleteSession}
            />
          ))
        )}
      </div>
    </div>
  );
}
