import { useState, type ReactNode } from "react";
import { Terminal } from "lucide-react";
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
  onCommitRequest,
  children,
}: LayoutProps) {
  const [rightPanelOpenLocal, setRightPanelOpenLocal] = useState(false);
  const rightPanelOpen = rightPanelOpenProp ?? rightPanelOpenLocal;
  const toggleRightPanel = onToggleRightPanel ?? (() => setRightPanelOpenLocal((v) => !v));

  const rightPanelContent = rightPanel ?? (
    <RightPanel
      diffs={rightPanelDiffs}
      connected={connected ?? false}
      sessionId={sessionId ?? null}
      todos={todos ?? []}
      gitInfo={gitInfo ?? null}
      onCommitRequest={onCommitRequest ?? (() => {})}
    />
  );

  const hamburgerIcon = (
    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
    </svg>
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
          {hamburgerIcon}
        </button>
      )}

      {/* Center content */}
      <main className="flex-1 flex flex-col min-w-0">
        {children}

        {/* Desktop: toolbar + cards */}
        {rightPanelOpen && (
          <div className="hidden md:block absolute top-3 right-3 z-10 w-[320px]">
            {/* Toolbar */}
            <div className="flex items-center justify-between px-2 py-1.5 rounded-xl border border-[var(--border-base)] bg-[var(--surface-secondary)] shadow-sm">
              <button
                onClick={toggleRightPanel}
                className="p-1 rounded-md hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
              >
                {hamburgerIcon}
              </button>
              <button
                className="p-1 rounded-md hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
                title="打开终端"
              >
                <Terminal className="w-4 h-4" />
              </button>
            </div>
            {/* Cards below toolbar */}
            <div className="mt-2 space-y-2 max-h-[calc(100vh-8rem)] overflow-y-auto">
              {rightPanelContent}
            </div>
          </div>
        )}
      </main>

      {/* Fixed toggle right button (visible when panel closed) */}
      {!rightPanelOpen && (
        <button
          onClick={toggleRightPanel}
          className="fixed top-3 z-50 p-1.5 rounded-md bg-neutral-800 hover:bg-neutral-700 text-neutral-400 hover:text-neutral-200 transition-colors"
          style={{ right: "8px" }}
          title="显示右侧面板"
        >
          {hamburgerIcon}
        </button>
      )}

      {/* Mobile drawer */}
      {rightPanelOpen && (
        <div className="md:hidden fixed inset-0 z-40 flex justify-end">
          <div className="fixed inset-0 bg-black/50" onClick={toggleRightPanel} />
          <aside className="relative w-[85vw] max-w-[320px] h-full flex flex-col bg-[var(--surface-secondary)] shadow-lg">
            <div className="flex items-center justify-between px-3 py-2 border-b border-[var(--border-base)]">
              <button
                onClick={toggleRightPanel}
                className="p-1 rounded-md hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
              >
                {hamburgerIcon}
              </button>
              <button className="p-1 rounded-md hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors">
                <Terminal className="w-4 h-4" />
              </button>
            </div>
            {rightPanelContent}
          </aside>
        </div>
      )}
    </div>
  );
}
