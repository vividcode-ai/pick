import { Puzzle, Settings, MessageSquarePlus, Pin, PinOff } from "lucide-react";
import { ThemeModeToggle } from "../ThemeModeToggle";
import { SessionList } from "./SessionList";

interface LeftPanelProps {
  onNewSession: () => void;
  onPlugins: () => void;
  onSettings: () => void;
  pinned?: boolean;
  onTogglePinned?: () => void;
  connected: boolean;
  activeSessionId: string | null;
  onSelectSession: (id: string) => void;
  onRenameSession: (id: string, title: string) => void;
  onArchiveSession: (id: string) => void;
  streamingSessions?: Record<string, boolean>;
}

export function LeftPanel({
  onNewSession,
  onPlugins,
  onSettings,
  pinned,
  onTogglePinned,
  connected,
  activeSessionId,
  onSelectSession,
  onRenameSession,
  onArchiveSession,
  streamingSessions,
}: LeftPanelProps) {
  return (
    <>
      <div className="flex items-center justify-around px-2 py-3 border-b border-neutral-800">
        <button
          onClick={onNewSession}
          title="New Session"
          className="p-2 rounded-md hover:bg-[var(--surface-hover)] text-neutral-400 hover:text-[var(--text-primary)] transition-colors"
        >
          <MessageSquarePlus className="w-5 h-5" />
        </button>
        <button
          onClick={onPlugins}
          title="Plugins"
          className="p-2 rounded-md hover:bg-[var(--surface-hover)] text-neutral-400 hover:text-[var(--text-primary)] transition-colors"
        >
          <Puzzle className="w-5 h-5" />
        </button>
        <ThemeModeToggle />
        {onTogglePinned && (
          <button
            onClick={onTogglePinned}
            title={pinned ? "Auto-close mode" : "Always show mode"}
            className="p-2 rounded-md hover:bg-[var(--surface-hover)] text-neutral-400 hover:text-[var(--text-primary)] transition-colors"
          >
            {pinned ? <Pin className="w-5 h-5" /> : <PinOff className="w-5 h-5" />}
          </button>
        )}
      </div>

      <SessionList
        activeSessionId={activeSessionId}
        onSelectSession={onSelectSession}
        onNewSession={onNewSession}
        onRenameSession={onRenameSession}
        onArchiveSession={onArchiveSession}
        streamingSessions={streamingSessions}
      />

      <div className="border-t px-3 py-3 space-y-2" style={{ borderColor: "var(--border-divider)" }}>
        <button
          onClick={onSettings}
          className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-[var(--surface-hover)] text-neutral-400 hover:text-[var(--text-primary)] text-sm transition-colors"
        >
          <Settings className="w-4 h-4" />
          Settings
        </button>
        <div className="flex items-center gap-2 text-xs text-neutral-500 px-2">
          <span className={`w-2 h-2 rounded-full ${connected ? "bg-green-500" : "bg-red-500"}`} />
          <span>{connected ? "Connected" : "Disconnected"}</span>
        </div>
      </div>
    </>
  );
}
