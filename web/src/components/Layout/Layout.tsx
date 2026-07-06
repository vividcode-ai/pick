import { useState, useEffect, useRef, type ReactNode } from "react";
import { Activity, Monitor } from "lucide-react";
import { RightPanel } from "./RightPanel";
import { TerminalPanel } from "./TerminalPanel";
import type { GitInfo, TodoItem } from "../../types/events";

interface LayoutProps {
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
  sidebarPinned: boolean;
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
  baseUrl?: string;
  onAsk?: ((prompt: string) => void) | null;
  children: ReactNode;
}

export function Layout({
  sidebarOpen,
  onToggleSidebar,
  sidebarPinned,
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
  baseUrl,
  onAsk,
  children,
}: LayoutProps) {
  const [rightPanelOpenLocal, setRightPanelOpenLocal] = useState(false);
  const rightPanelOpen = rightPanelOpenProp ?? rightPanelOpenLocal;
  const toggleRightPanel = onToggleRightPanel ?? (() => setRightPanelOpenLocal((v) => !v));
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [terminalFullscreen, setTerminalFullscreen] = useState(false);
  const toggleTerminal = () => setTerminalOpen((v) => !v);

  const [isMobile, setIsMobile] = useState(false);
  useEffect(() => {
    const check = () => setIsMobile(window.innerWidth < 768);
    check();
    window.addEventListener("resize", check);
    return () => window.removeEventListener("resize", check);
  }, []);

  const [sidebarWidthPercent, setSidebarWidthPercent] = useState(() => {
    try {
      const saved = localStorage.getItem("pick_sidebar_width");
      if (saved) {
        const n = Number(saved);
        if (!isNaN(n)) return Math.max(10, Math.min(30, n));
      }
    } catch {}
    return 15;
  });

  const [isResizing, setIsResizing] = useState(false);
  const resizeStartX = useRef(0);
  const resizeStartWidth = useRef(0);
  const currentWidthRef = useRef(sidebarWidthPercent);
  currentWidthRef.current = sidebarWidthPercent;

  const handleResizeStart = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsResizing(true);
    resizeStartX.current = e.clientX;
    resizeStartWidth.current = sidebarWidthPercent;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  };

  useEffect(() => {
    if (!isResizing) return;
    const handleMouseMove = (e: MouseEvent) => {
      const deltaX = e.clientX - resizeStartX.current;
      const newWidth = Math.max(10, Math.min(30,
        resizeStartWidth.current + (deltaX / window.innerWidth) * 100
      ));
      setSidebarWidthPercent(newWidth);
    };
    const handleMouseUp = () => {
      setIsResizing(false);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      localStorage.setItem("pick_sidebar_width", String(currentWidthRef.current));
    };
    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isResizing]);

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

  const isDrawer = !sidebarPinned || isMobile;

  return (
    <div className="flex h-screen overflow-hidden bg-[var(--surface-base)] text-[var(--text-primary)]">
      {isDrawer && sidebarOpen && (
        <div
          className="fixed inset-0 z-30 bg-black/50"
          onClick={onToggleSidebar}
        />
      )}

      <aside
        className={`${
          isDrawer
            ? `${sidebarOpen ? "translate-x-0" : "-translate-x-full"} fixed z-40 transition-transform duration-200 ease-in-out`
            : "relative"
        } flex flex-col h-full border-r border-[var(--border-base)] bg-[var(--surface-secondary)]`}
        style={{
          width: `${sidebarWidthPercent}%`,
          minWidth: "max(10vw, 180px)",
          maxWidth: "30vw",
        }}
      >
        {leftPanel}

        <div
          className="absolute right-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-[var(--accent-primary)] active:bg-[var(--accent-primary)] transition-colors z-10"
          onMouseDown={handleResizeStart}
        />
      </aside>

      {isDrawer && !sidebarOpen && (
        <button
          onClick={onToggleSidebar}
          className="fixed top-3 z-50 p-1.5 rounded-md bg-neutral-800 hover:bg-neutral-700 text-neutral-400 hover:text-neutral-200 transition-colors"
          style={{ left: "8px" }}
        >
          {hamburgerIcon}
        </button>
      )}

      <main className="flex-1 flex flex-col min-w-0">
        <div className={terminalFullscreen ? "hidden" : "flex-1 flex flex-col min-h-0"}>
          {children}
        </div>

        {baseUrl && terminalOpen && (
          <TerminalPanel
            baseUrl={baseUrl}
            visible={true}
            onClose={() => setTerminalOpen(false)}
            onFullscreenChange={setTerminalFullscreen}
            sessionId={sessionId ?? null}
            onAsk={onAsk}
          />
        )}

        {!terminalFullscreen && (
          <div className="hidden md:block fixed top-3 right-3 z-20">
            <div className="flex items-center justify-between gap-2 px-2 py-1.5 rounded-xl border border-[var(--border-base)] bg-[var(--surface-secondary)] shadow-sm w-fit">
              <button
                onClick={toggleRightPanel}
                className="p-1 rounded-md hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
              >
                <Activity className="w-4 h-4" />
              </button>
              <button
                onClick={toggleTerminal}
                className={`p-1 rounded-md hover:bg-[var(--surface-hover)] transition-colors ${
                  terminalOpen
                    ? "text-[var(--accent-primary)] bg-[var(--surface-hover)]"
                    : "text-[var(--text-muted)] hover:text-[var(--text-primary)]"
                }`}
                title="Open Terminal"
              >
                <Monitor className="w-4 h-4" />
              </button>
            </div>
          </div>
        )}

        {rightPanelOpen && (
          <div className="hidden md:block fixed top-3 right-3 z-10" style={{ marginTop: "44px" }}>
            <div className="mt-2 space-y-2 max-h-[calc(100vh-8rem)] overflow-y-auto w-[320px]">
              {rightPanelContent}
            </div>
          </div>
        )}
      </main>

      {rightPanelOpen && (
        <div className="md:hidden fixed inset-0 z-40 flex justify-end">
          <div className="fixed inset-0 bg-black/50" onClick={toggleRightPanel} />
          <aside className="relative w-[85vw] max-w-[320px] h-full flex flex-col bg-[var(--surface-secondary)] shadow-lg">
            <div className="flex items-center justify-between px-3 py-2 border-b border-[var(--border-base)]">
              <button
                onClick={toggleRightPanel}
                className="p-1 rounded-md hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
              >
                <Activity className="w-4 h-4" />
              </button>
              <button
                onClick={toggleTerminal}
                className={`p-1 rounded-md hover:bg-[var(--surface-hover)] transition-colors ${
                  terminalOpen
                    ? "text-[var(--accent-primary)] bg-[var(--surface-hover)]"
                    : "text-[var(--text-muted)] hover:text-[var(--text-primary)]"
                }`}
                title="Open Terminal"
              >
                <Monitor className="w-4 h-4" />
              </button>
            </div>
            {rightPanelContent}
          </aside>
        </div>
      )}
    </div>
  );
}
