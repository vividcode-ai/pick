import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { useTheme } from "../../lib/ThemeProvider";

const MIN_HEIGHT = 100;
const MAX_HEIGHT_FRACTION = 0.6;

interface TerminalPanelProps {
  baseUrl: string;
  visible: boolean;
  onClose: () => void;
  onFullscreenChange?: (fullscreen: boolean) => void;
}

export function TerminalPanel({ baseUrl, visible, onClose, onFullscreenChange }: TerminalPanelProps) {
  const [tabs, setTabs] = useState<{ id: number; shellName?: string }[]>([{ id: 0 }]);
  const [activeIdx, setActiveIdx] = useState(0);
  const [panelHeight, setPanelHeight] = useState(200);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const prevHeightRef = useRef(200);

  const containerRefs = useRef<Map<number, HTMLDivElement>>(new Map());
  const termMapRef = useRef<Map<number, Terminal>>(new Map());
  const fitAddonMapRef = useRef<Map<number, FitAddon>>(new Map());

  const wsMapRef = useRef<Map<number, WebSocket>>(new Map());
  const activeWsRef = useRef<WebSocket | null>(null);

  const draggingRef = useRef(false);

  const tabListRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);

  const { isDark } = useTheme();

  const termTheme = useMemo(() => ({
    background: isDark ? "#1e1e2e" : "#ffffff",
    foreground: isDark ? "#cdd6f4" : "#000000",
    cursor: isDark ? "#f5e0dc" : "#000000",
    selectionBackground: isDark ? "#585b70" : "#c0c0c0",
    black: isDark ? "#45475a" : "#000000",
    red: isDark ? "#f38ba8" : "#c41a16",
    green: isDark ? "#a6e3a1" : "#007d1a",
    yellow: isDark ? "#f9e2af" : "#9d7d1a",
    blue: isDark ? "#89b4fa" : "#0451a5",
    magenta: isDark ? "#f5c2e7" : "#a304a3",
    cyan: isDark ? "#94e2d5" : "#058a8a",
    white: isDark ? "#bac2de" : "#c0c0c0",
    brightBlack: isDark ? "#585b70" : "#666666",
    brightRed: isDark ? "#f38ba8" : "#c41a16",
    brightGreen: isDark ? "#a6e3a1" : "#007d1a",
    brightYellow: isDark ? "#f9e2af" : "#9d7d1a",
    brightBlue: isDark ? "#89b4fa" : "#0451a5",
    brightMagenta: isDark ? "#f5c2e7" : "#a304a3",
    brightCyan: isDark ? "#94e2d5" : "#058a8a",
    brightWhite: isDark ? "#a6adc8" : "#d4d4d4",
  }), [isDark]);

  const activeTabId = tabs[activeIdx]?.id;

  const wsUrl = useMemo(() => {
    try {
      const url = new URL(baseUrl);
      url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
      url.pathname = "/pty-ws";
      return url.toString();
    } catch {
      return "ws://127.0.0.1/pty-ws";
    }
  }, [baseUrl]);

  // Lazy-init terminal when active tab changes (and panel is visible)
  useEffect(() => {
    if (!visible) return;
    const id = activeTabId;
    if (id == null) return;
    if (termMapRef.current.has(id)) return;

    const container = containerRefs.current.get(id);
    if (!container) return;

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: "'Cascadia Code', 'Fira Code', 'Consolas', monospace",
      theme: termTheme,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(container);
    term.focus();
    setTimeout(() => {
      fitAddon.fit();
      term.focus();
    }, 50);

    const unbindData = term.onData((data) => {
      const ws = activeWsRef.current;
      if (ws?.readyState === WebSocket.OPEN) {
        ws.send(data);
      }
    });
    const unbindResize = term.onResize(({ cols, rows }) => {
      const ws = activeWsRef.current;
      if (ws?.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: "resize", cols, rows }));
      }
    });

    termMapRef.current.set(id, term);
    fitAddonMapRef.current.set(id, fitAddon);

    // Clean up if tab is deleted while effect runs
    return () => {
      unbindData.dispose();
      unbindResize.dispose();
      // Only dispose if the tab still exists (avoids double-dispose in deleteTab)
      if (termMapRef.current.get(id) === term) {
        term.dispose();
        termMapRef.current.delete(id);
        fitAddonMapRef.current.delete(id);
      }
    };
  }, [visible, activeTabId]);

  // Update terminal themes when app theme changes
  useEffect(() => {
    for (const term of termMapRef.current.values()) {
      term.options.theme = termTheme;
    }
  }, [termTheme]);

  // Fit terminal on window resize
  useEffect(() => {
    if (!visible) return;
    const onResize = () => {
      const id = tabs[activeIdx]?.id;
      if (id == null) return;
      fitAddonMapRef.current.get(id)?.fit();
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [visible, activeIdx, tabs]);

  // Init first tab's WS when panel opens
  useEffect(() => {
    if (!visible || tabs.length === 0) return;
    const firstId = tabs[0].id;
    if (!wsMapRef.current.has(firstId)) {
      const ws = createWs(firstId);
      activeWsRef.current = ws;
    }
  }, [visible]);

  // Refocus terminal when panel becomes visible
  useEffect(() => {
    if (!visible) return;
    const id = tabs[activeIdx]?.id;
    if (id == null) return;
    const term = termMapRef.current.get(id);
    term?.focus();
  }, [visible, tabs, activeIdx]);
  const decodeBuffer = (buf: ArrayBuffer): string => {
    try {
      return new TextDecoder("utf-8", { fatal: true }).decode(buf);
    } catch {
      return new TextDecoder("gbk").decode(buf);
    }
  };

  const createWs = useCallback(
    (tabId: number): WebSocket => {
      const ws = new WebSocket(wsUrl);

      ws.onmessage = (event) => {
        const term = termMapRef.current.get(tabId);
        if (!term) return;

        if (typeof event.data === "string") {
          const m = event.data.match(/\*\*\* Pick Terminal \(.*[/\\]([^/\\]+)\):/);
          if (m) {
            const name = m[1].replace(/\.exe$/i, "");
            const displayName = name.charAt(0).toUpperCase() + name.slice(1);
            setTabs((prev) =>
              prev.map((t) => (t.id === tabId ? { ...t, shellName: displayName } : t))
            );
          }
        }

        if (event.data instanceof Blob) {
          event.data.arrayBuffer().then((buf) => {
            term.write(decodeBuffer(buf).replace(/\n/g, "\r\n"));
          });
        } else {
          term.write(event.data.replace(/\n/g, "\r\n"));
        }
      };
      ws.onclose = () => {
        const term = termMapRef.current.get(tabId);
        term?.write("\r\n\x1b[31m[Connection closed]\x1b[0m\r\n");
      };
      ws.onerror = () => {
        const term = termMapRef.current.get(tabId);
        term?.write("\r\n\x1b[31m[Connection error]\x1b[0m\r\n");
      };

      wsMapRef.current.set(tabId, ws);
      return ws;
    },
    [wsUrl],
  );

  // ── Actions ──────────────────────────────────────────────────────

  const handleNew = useCallback(() => {
    const newId = tabs.length > 0 ? Math.max(...tabs.map((t) => t.id)) + 1 : 0;
    const ws = createWs(newId);
    activeWsRef.current = ws;
    setTabs((prev) => [...prev, { id: newId }]);
    setActiveIdx(tabs.length);
  }, [tabs, createWs]);

  const handleSelect = useCallback(
    (idx: number) => {
      const tab = tabs[idx];
      if (!tab) return;
      const ws = wsMapRef.current.get(tab.id);
      if (ws) {
        activeWsRef.current = ws;
      }
      setActiveIdx(idx);
      const term = termMapRef.current.get(tab.id);
      term?.focus();
    },
    [tabs],
  );

  const handleDeleteTab = useCallback(
    (idx: number) => {
      if (tabs.length <= 1) return;
      const tab = tabs[idx];
      wsMapRef.current.get(tab.id)?.close();
      wsMapRef.current.delete(tab.id);
      termMapRef.current.get(tab.id)?.dispose();
      termMapRef.current.delete(tab.id);
      fitAddonMapRef.current.delete(tab.id);
      containerRefs.current.delete(tab.id);
      const newTabs = tabs.filter((_, i) => i !== idx);
      const newIdx = Math.min(idx, newTabs.length - 1);
      const newActiveTab = newTabs[newIdx];
      if (newActiveTab) {
        const ws = wsMapRef.current.get(newActiveTab.id);
        if (ws) activeWsRef.current = ws;
      }
      setTabs(newTabs);
      setActiveIdx(newIdx);
    },
    [tabs],
  );

  const handleFullscreen = useCallback(() => {
    setIsFullscreen((prev) => {
      const next = !prev;
      onFullscreenChange?.(next);
      if (next) {
        prevHeightRef.current = panelHeight;
        setTimeout(() => {
          const id = tabs[activeIdx]?.id;
          if (id != null) fitAddonMapRef.current.get(id)?.fit();
        }, 50);
      } else {
        setPanelHeight(prevHeightRef.current);
        setTimeout(() => {
          const id = tabs[activeIdx]?.id;
          if (id != null) fitAddonMapRef.current.get(id)?.fit();
        }, 50);
      }
      return next;
    });
  }, [panelHeight, onFullscreenChange, tabs, activeIdx]);

  const scrollTabs = useCallback((direction: number) => {
    tabListRef.current?.scrollBy({ left: direction * 150, behavior: "smooth" });
  }, []);

  const updateScrollButtons = useCallback(() => {
    const el = tabListRef.current;
    if (!el) return;
    setCanScrollLeft(el.scrollLeft > 0);
    setCanScrollRight(el.scrollLeft + el.clientWidth < el.scrollWidth);
  }, []);

  // Track scroll position for tab arrow buttons
  useEffect(() => {
    const el = tabListRef.current;
    if (!el) return;
    const cb = () => updateScrollButtons();
    el.addEventListener("scroll", cb);
    updateScrollButtons();
    return () => el.removeEventListener("scroll", cb);
  }, [tabs.length, updateScrollButtons]);

  // Check overflow on resize
  useEffect(() => {
    const el = tabListRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => updateScrollButtons());
    ro.observe(el);
    return () => ro.disconnect();
  }, [updateScrollButtons]);

  // ── Drag resize ──────────────────────────────────────────────────

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (isFullscreen) return;
      e.preventDefault();
      draggingRef.current = true;
      const startY = e.clientY;
      const startHeight = panelHeight;

      const handleMouseMove = (ev: MouseEvent) => {
        if (!draggingRef.current) return;
        const delta = startY - ev.clientY;
        const newHeight = Math.max(
          MIN_HEIGHT,
          Math.min(window.innerHeight * MAX_HEIGHT_FRACTION, startHeight + delta),
        );
        setPanelHeight(newHeight);
      };

      const handleMouseUp = () => {
        if (!draggingRef.current) return;
        draggingRef.current = false;
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
        const id = tabs[activeIdx]?.id;
        if (id != null) fitAddonMapRef.current.get(id)?.fit();
      };

      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    },
    [isFullscreen, panelHeight, tabs, activeIdx],
  );

  // ── Render ───────────────────────────────────────────────────────

  return (
    <div
      className={`border-t border-[var(--border-base)] ${
        isDark ? "bg-[#1e1e2e]" : "bg-white"
      } ${isFullscreen ? "flex flex-col flex-1" : ""}`}
      style={{ display: visible ? (isFullscreen ? "flex" : "block") : "none" }}
    >
      {/* Resize handle (hidden in fullscreen) */}
      {!isFullscreen && (
        <div
          onMouseDown={handleMouseDown}
          className="h-[5px] cursor-row-resize bg-transparent hover:bg-[var(--accent-primary)] transition-colors"
        />
      )}

      {/* Title bar */}
      <div className={`flex items-center ${
        isDark ? "bg-[#181825]" : "bg-[#f3f3f3]"
      } border-b border-[var(--border-base)] flex-shrink-0`}>
        {/* Scroll left */}
        {canScrollLeft && (
          <button
            onClick={() => scrollTabs(-1)}
            className="self-stretch px-1 hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 18l-6-6 6-6" />
            </svg>
          </button>
        )}

        {/* Tab list */}
        <div
          ref={tabListRef}
          className="flex items-center gap-0 flex-1 min-w-0 self-stretch overflow-hidden"
        >
          {tabs.map((tab, i) => (
            <div
              key={tab.id}
              className={`group flex items-center gap-1 px-3 text-xs whitespace-nowrap cursor-pointer border-b-2 transition-colors select-none shrink-0 ${
                i === activeIdx
                  ? "text-[var(--text-primary)] border-b-[var(--accent-primary)]"
                  : "text-[var(--text-muted)] hover:text-[var(--text-primary)] border-b-transparent hover:border-b-[var(--border-base)]"
              }`}
              onClick={() => handleSelect(i)}
            >
              <span className="py-1">T{tab.id + 1}: {tab.shellName || "..."}</span>
              <button
                onClick={(e) => { e.stopPropagation(); handleDeleteTab(i); }}
                className="p-0.5 rounded hover:bg-red-500/20 text-[var(--text-muted)] hover:text-red-400 transition-colors"
                title="Delete terminal"
              >
                <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          ))}
          <button
            onClick={handleNew}
            title="New terminal"
            className="self-stretch px-2 hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors shrink-0"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
            </svg>
          </button>
        </div>

        {/* Scroll right */}
        {canScrollRight && (
          <button
            onClick={() => scrollTabs(1)}
            className="self-stretch px-1 hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 18l6-6-6-6" />
            </svg>
          </button>
        )}

        {/* Action buttons */}
        <div className="flex items-center gap-1 px-1 ml-auto">
          {/* Fullscreen toggle */}
          <button
            onClick={handleFullscreen}
            title={isFullscreen ? "Exit fullscreen" : "Fullscreen"}
            className={`p-0.5 rounded hover:bg-[var(--surface-hover)] transition-colors ${
              isFullscreen
                ? "text-[var(--accent-primary)] bg-[var(--surface-hover)]"
                : "text-[var(--text-muted)] hover:text-[var(--text-primary)]"
            }`}
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4"
              />
            </svg>
          </button>

          {/* Close panel */}
          <button
            onClick={onClose}
            title="Close terminal"
            className="p-0.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      {/* Terminal body: xterm + tab list */}
      <div className="flex flex-1 min-h-0" style={isFullscreen ? {} : { height: panelHeight }}>
        {/* Per-tab terminal containers */}
        <div className="flex-1 min-w-0 relative">
          {tabs.map((tab) => (
            <div
              key={tab.id}
              ref={(el) => {
                if (el) containerRefs.current.set(tab.id, el);
              }}
              className="absolute inset-0"
              style={{ display: tab.id === activeTabId ? "block" : "none" }}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
