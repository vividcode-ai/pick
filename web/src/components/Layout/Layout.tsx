import { useState, type ReactNode } from "react";
import { Pin, PinOff } from "lucide-react";
import { RightPanel } from "./RightPanel";
import type { GitInfo, TodoItem } from "../../types/events";

interface LayoutProps {
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
  sidebarPinned: boolean;
  onToggleSidebarPinned: () => void;
  leftPanel: ReactNode;
  rightPanel?: ReactNode;
  rightPanelDiffs?: { filePath: string; content: string }[];
  connected?: boolean;
  rightPanelOpen?: boolean;
  onToggleRightPanel?: () => void;
  sessionId?: string | null;
  todos?: TodoItem[];
  gitInfo?: GitInfo | null;
  baseUrl?: string;
  onCommitRequest?: (message: string) => void;
  children: ReactNode;
}

export function Layout({
  sidebarOpen,
  onToggleSidebar,
  sidebarPinned,
  onToggleSidebarPinned,
  leftPanel,
  rightPanel,
  rightPanelDiffs,
  connected,
  rightPanelOpen: rightPanelOpenProp,
  onToggleRightPanel,
  sessionId,
  todos,
  gitInfo,
  baseUrl,
  onCommitRequest,
  children,
}: LayoutProps) {
  const [rightPanelOpenLocal, setRightPanelOpenLocal] = useState(false);
  const rightPanelOpen = rightPanelOpenProp ?? rightPanelOpenLocal;
  const toggleRightPanel = onToggleRightPanel ?? (() => setRightPanelOpenLocal((v) => !v));

  const [rightPanelPinned, setRightPanelPinned] = useState(true);
  const toggleRightPanelPinned = () => setRightPanelPinned((v) => !v);

  const rightPanelContent = rightPanel ?? (
    <RightPanel
      diffs={rightPanelDiffs}
      connected={connected ?? false}
      sessionId={sessionId ?? null}
      todos={todos ?? []}
      gitInfo={gitInfo ?? null}
      baseUrl={baseUrl ?? ""}
      onCommitRequest={onCommitRequest ?? (() => {})}
    />
  );

  return (
    <div className="flex h-screen overflow-hidden bg-[var(--surface-base)] text-[var(--text-primary)]">
      {/* Mobile overlay */}
      {sidebarOpen && (
        <div
          className="fixed inset-0 z-30 bg-black/50 md:hidden"
          onClick={onToggleSidebar}
        />
      )}

      {/* Left Panel */}
      <aside
        className={`${
          sidebarOpen ? "translate-x-0" : "-translate-x-full"
        } fixed md:static z-40 h-full transition-transform duration-200 ease-in-out
        w-[80vw] max-w-[280px] md:w-[10%] md:min-w-[180px] md:max-w-[280px]
        flex flex-col border-r border-[var(--border-base)] bg-[var(--surface-secondary)]`}
      >
        {leftPanel}
      </aside>

      {/* Toggle left button (visible when sidebar closed) */}
      {!sidebarOpen && (
        <button
          onClick={onToggleSidebar}
          className="fixed top-3 z-50 p-1.5 rounded-md bg-neutral-800 hover:bg-neutral-700 text-neutral-400 hover:text-neutral-200 transition-colors md:top-3"
          style={{ left: "8px" }}
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
          </svg>
        </button>
      )}

      {/* Center content */}
      <main
        className="flex-1 flex flex-col min-w-0 relative"
        onClick={() => {
          if (sidebarOpen && !sidebarPinned) onToggleSidebar();
        }}
      >
        {children}

        {/* Floating Right Panel (desktop) */}
        {rightPanelOpen && (
          <div className="hidden md:block absolute top-3 right-3 z-10 w-[320px] max-h-[calc(100vh-6rem)]">
            <div className="flex flex-col h-full rounded-xl border border-[var(--border-base)] bg-[var(--surface-secondary)] shadow-lg overflow-hidden">
              <div className="flex items-center justify-end px-3 py-2 border-b border-[var(--border-base)] flex-shrink-0">
                <button
                  onClick={toggleRightPanelPinned}
                  className="p-1.5 rounded-md hover:bg-neutral-700 text-neutral-400 hover:text-neutral-200 transition-colors"
                  title={rightPanelPinned ? "Auto-close" : "Keep open"}
                >
                  {rightPanelPinned ? <Pin className="w-4 h-4" /> : <PinOff className="w-4 h-4" />}
                </button>
              </div>
              <div className="flex-1 overflow-y-auto min-h-0">
                {rightPanelContent}
              </div>
            </div>
          </div>
        )}

        {/* Toggle right button (visible when panel closed) */}
        {!rightPanelOpen && (
          <button
            onClick={toggleRightPanel}
            className="absolute top-3 right-3 z-10 p-1.5 rounded-md bg-neutral-800 hover:bg-neutral-700 text-neutral-400 hover:text-neutral-200 transition-colors"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
            </svg>
          </button>
        )}
      </main>

      {/* Right Panel (mobile drawer) */}
      {rightPanelOpen && (
        <div className="md:hidden fixed inset-0 z-40 flex justify-end">
          <div className="fixed inset-0 bg-black/50" onClick={toggleRightPanel} />
          <aside className="relative w-[85vw] max-w-[320px] h-full flex flex-col bg-[var(--surface-secondary)] shadow-lg">
            <div className="flex items-center justify-end px-3 py-2 border-b border-[var(--border-base)]">
              <button
                onClick={toggleRightPanelPinned}
                className="p-1.5 rounded-md hover:bg-neutral-700 text-neutral-400 hover:text-neutral-200 transition-colors"
                title={rightPanelPinned ? "Auto-close" : "Keep open"}
              >
                {rightPanelPinned ? <Pin className="w-4 h-4" /> : <PinOff className="w-4 h-4" />}
              </button>
            </div>
            {rightPanelContent}
          </aside>
        </div>
      )}
    </div>
  );
}
