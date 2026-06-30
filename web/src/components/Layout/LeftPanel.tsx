interface LeftPanelProps {
  onNewSession: () => void;
  onSearch: () => void;
  onPlugins: () => void;
  onSettings: () => void;
  connected: boolean;
}

export function LeftPanel({
  onNewSession,
  onSearch,
  onPlugins,
  onSettings,
  connected,
}: LeftPanelProps) {
  return (
    <>
      {/* Top section: action buttons */}
      <div className="flex items-center justify-around px-2 py-3 border-b border-neutral-800">
        <button
          onClick={onNewSession}
          title="New Session"
          className="p-2 rounded-md hover:bg-neutral-800 text-neutral-400 hover:text-neutral-200 transition-colors"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
        </button>
        <button
          onClick={onSearch}
          title="Search Sessions"
          className="p-2 rounded-md hover:bg-neutral-800 text-neutral-400 hover:text-neutral-200 transition-colors"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
        </button>
        <button
          onClick={onPlugins}
          title="Plugins"
          className="p-2 rounded-md hover:bg-neutral-800 text-neutral-400 hover:text-neutral-200 transition-colors"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 4a2 2 0 114 0v1a1 1 0 001 1h3a1 1 0 011 1v3a1 1 0 01-1 1h-1a2 2 0 100 4h1a1 1 0 011 1v3a1 1 0 01-1 1h-3a1 1 0 01-1-1v-1a2 2 0 10-4 0v1a1 1 0 01-1 1H7a1 1 0 01-1-1v-3a1 1 0 00-1-1H4a2 2 0 110-4h1a1 1 0 001-1V7a1 1 0 011-1h3a1 1 0 001-1V4z" />
          </svg>
        </button>
      </div>

      {/* Middle section: session list */}
      <div className="flex-1 overflow-y-auto px-2 py-2 space-y-1">
        <div className="text-xs text-neutral-500 text-center py-8">
          No sessions yet
        </div>
      </div>

      {/* Bottom section: settings + status */}
      <div className="border-t border-neutral-800 px-3 py-3 space-y-2">
        <button
          onClick={onSettings}
          className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-neutral-800 text-neutral-400 hover:text-neutral-200 text-sm transition-colors"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
          Settings
        </button>
        <div className="flex items-center gap-2 text-xs text-neutral-500 px-2">
          <span
            className={`w-2 h-2 rounded-full ${
              connected ? "bg-green-500" : "bg-red-500"
            }`}
          />
          <span>{connected ? "Connected" : "Disconnected"}</span>
        </div>
      </div>
    </>
  );
}
