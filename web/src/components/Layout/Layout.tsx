import { useState, type ReactNode } from "react";
import { RightPanel } from "./RightPanel";

interface LayoutProps {
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
  leftPanel: ReactNode;
  rightPanel?: ReactNode;
  rightPanelDiffs?: { filePath: string; content: string }[];
  connected?: boolean;
  rightPanelOpen?: boolean;
  onToggleRightPanel?: () => void;
  children: ReactNode;
}

export function Layout({
  sidebarOpen,
  onToggleSidebar,
  leftPanel,
  rightPanel,
  rightPanelDiffs,
  connected,
  rightPanelOpen: rightPanelOpenProp,
  onToggleRightPanel,
  children,
}: LayoutProps) {
  const [rightPanelOpenLocal, setRightPanelOpenLocal] = useState(false);
  const rightPanelOpen = rightPanelOpenProp ?? rightPanelOpenLocal;
  const toggleRightPanel = onToggleRightPanel ?? (() => setRightPanelOpenLocal((v) => !v));

  const rightPanelContent = rightPanel ?? <RightPanel diffs={rightPanelDiffs} connected={connected ?? false} />;

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

      {/* Toggle left button */}
      <button
        onClick={onToggleSidebar}
        className="fixed top-3 z-50 p-1.5 rounded-md bg-neutral-800 hover:bg-neutral-700 text-neutral-400 hover:text-neutral-200 transition-colors md:top-3"
        style={{
          left: sidebarOpen ? "calc(max(10%, 180px) + 8px)" : "8px",
        }}
      >
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          {sidebarOpen ? (
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          ) : (
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
          )}
        </svg>
      </button>

      {/* Center content */}
      <main className="flex-1 flex flex-col min-w-0">
        {children}
      </main>

      {/* Toggle right button */}
      <button
        onClick={toggleRightPanel}
        className="fixed top-3 z-50 p-1.5 rounded-md bg-neutral-800 hover:bg-neutral-700 text-neutral-400 hover:text-neutral-200 transition-colors"
        style={{ right: rightPanelOpen ? "calc(30vw + 8px)" : "8px" }}
      >
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
        </svg>
      </button>

      {/* Right Panel (desktop pinned) */}
      {rightPanelOpen && (
        <aside
          className="hidden md:flex w-[30vw] min-w-[280px] max-w-[400px] flex-col border-l border-[var(--border-base)] bg-[var(--surface-secondary)]"
        >
          {rightPanelContent}
        </aside>
      )}

      {/* Right Panel (mobile drawer) */}
      {rightPanelOpen && (
        <div className="md:hidden fixed inset-0 z-40 flex justify-end">
          <div className="fixed inset-0 bg-black/50" onClick={toggleRightPanel} />
          <aside className="relative w-[85vw] max-w-[320px] h-full flex flex-col bg-[var(--surface-secondary)] shadow-lg">
            {rightPanelContent}
          </aside>
        </div>
      )}
    </div>
  );
}
