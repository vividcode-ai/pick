import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

const MIN_HEIGHT = 100;
const MAX_HEIGHT_FRACTION = 0.6;

interface TerminalPanelProps {
  baseUrl: string;
  visible: boolean;
  onClose: () => void;
  onFullscreenChange?: (fullscreen: boolean) => void;
}

export function TerminalPanel({ baseUrl, visible, onClose, onFullscreenChange }: TerminalPanelProps) {
  const [tabs, setTabs] = useState<{ id: number }[]>([{ id: 0 }]);
  const [activeIdx, setActiveIdx] = useState(0);
  const [panelHeight, setPanelHeight] = useState(200);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const prevHeightRef = useRef(200);

  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);

  // Multi-WS: map of tab-id → WebSocket, plus ref to the currently active WS
  const wsMapRef = useRef<Map<number, WebSocket>>(new Map());
  const activeWsRef = useRef<WebSocket | null>(null);

  const draggingRef = useRef(false);

  // Build WS URL once
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

  // Initialize xterm once
  useEffect(() => {
    if (!visible || !containerRef.current) return;

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: "'Cascadia Code', 'Fira Code', 'Consolas', monospace",
      theme: {
        background: "#1e1e2e",
        foreground: "#cdd6f4",
        cursor: "#f5e0dc",
        selectionBackground: "#585b70",
        black: "#45475a",
        red: "#f38ba8",
        green: "#a6e3a1",
        yellow: "#f9e2af",
        blue: "#89b4fa",
        magenta: "#f5c2e7",
        cyan: "#94e2d5",
        white: "#bac2de",
        brightBlack: "#585b70",
        brightRed: "#f38ba8",
        brightGreen: "#a6e3a1",
        brightYellow: "#f9e2af",
        brightBlue: "#89b4fa",
        brightMagenta: "#f5c2e7",
        brightCyan: "#94e2d5",
        brightWhite: "#a6adc8",
      },
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(containerRef.current);
    setTimeout(() => fitAddon.fit(), 50);
    termRef.current = term;
    fitAddonRef.current = fitAddon;

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

    const onResize = () => fitAddon.fit();
    window.addEventListener("resize", onResize);

    return () => {
      window.removeEventListener("resize", onResize);
      unbindData.dispose();
      unbindResize.dispose();
      term.dispose();
      termRef.current = null;
    };
  }, [visible]);

  // Helper: create a WS and wire it to write into the current xterm
  const createWs = useCallback(
    (tabId: number): WebSocket => {
      const ws = new WebSocket(wsUrl);

      const decodeBuffer = (buf: ArrayBuffer): string => {
        try {
          return new TextDecoder("utf-8", { fatal: true }).decode(buf);
        } catch {
          return new TextDecoder("gbk").decode(buf);
        }
      };

      ws.onmessage = (event) => {
        if (event.data instanceof Blob) {
          event.data.arrayBuffer().then((buf) => {
            termRef.current?.write(decodeBuffer(buf).replace(/\n/g, "\r\n"));
          });
        } else {
          termRef.current?.write(event.data.replace(/\n/g, "\r\n"));
        }
      };
      ws.onclose = () => {
        termRef.current?.write("\r\n\x1b[31m[Connection closed]\x1b[0m\r\n");
      };
      ws.onerror = () => {
        termRef.current?.write("\r\n\x1b[31m[Connection error]\x1b[0m\r\n");
      };

      wsMapRef.current.set(tabId, ws);
      return ws;
    },
    [wsUrl],
  );

  // Init first tab's WS when terminal panel opens (one-shot)
  useEffect(() => {
    if (!visible || tabs.length === 0) return;
    const firstId = tabs[0].id;
    if (!wsMapRef.current.has(firstId)) {
      const ws = createWs(firstId);
      activeWsRef.current = ws;
    }
    // Intentionally only run on visible change
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visible]);

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
        termRef.current?.reset();
      }
      setActiveIdx(idx);
    },
    [tabs],
  );

  const handleDeleteTab = useCallback(
    (idx: number) => {
      if (tabs.length <= 1) return;
      const tab = tabs[idx];
      const ws = wsMapRef.current.get(tab.id);
      ws?.close();
      wsMapRef.current.delete(tab.id);
      if (activeWsRef.current === ws) {
        activeWsRef.current = null;
      }
      setTabs((prev) => prev.filter((_, i) => i !== idx));
      setActiveIdx((prev) => Math.min(prev, tabs.length - 2));
    },
    [tabs],
  );

  const handleDelete = useCallback(() => {
    if (tabs.length <= 1) return;
    handleDeleteTab(activeIdx);
  }, [tabs, activeIdx, handleDeleteTab]);

  const handleFullscreen = useCallback(() => {
    setIsFullscreen((prev) => {
      const next = !prev;
      onFullscreenChange?.(next);
      if (next) {
        prevHeightRef.current = panelHeight;
        setTimeout(() => fitAddonRef.current?.fit(), 50);
      } else {
        setPanelHeight(prevHeightRef.current);
        setTimeout(() => fitAddonRef.current?.fit(), 50);
      }
      return next;
    });
  }, [panelHeight, onFullscreenChange]);

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
        fitAddonRef.current?.fit();
      };

      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    },
    [isFullscreen, panelHeight],
  );

  // ── Render ───────────────────────────────────────────────────────

  return (
    <div
      className={`border-t border-[var(--border-base)] bg-[#1e1e2e] ${
        isFullscreen ? "flex flex-col flex-1" : ""
      }`}
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
      <div className="flex items-center justify-between px-3 py-1 bg-[#181825] border-b border-[var(--border-base)] flex-shrink-0">
        <span className="text-xs text-[var(--text-muted)] font-medium">Terminal</span>
        <div className="flex items-center gap-1">
          {/* New tab */}
          <button
            onClick={handleNew}
            title="New terminal"
            className="p-0.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
            </svg>
          </button>

          {/* Delete tab */}
          {tabs.length > 1 && (
            <button
              onClick={handleDelete}
              title="Delete terminal"
              className="p-0.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                />
              </svg>
            </button>
          )}

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
      <div className="flex flex-row flex-1 min-h-0" style={isFullscreen ? {} : { height: panelHeight }}>
        <div ref={containerRef} className="flex-1 min-w-0" />

        {/* Tab list (right side, only when 2+ tabs) */}
        {tabs.length > 1 && (
          <div className="flex flex-col border-l border-[var(--border-base)] bg-[#181825] overflow-y-auto">
            {tabs.map((tab, i) => (
              <div
                key={tab.id}
                className={`flex items-center gap-1 px-2 py-1.5 text-xs whitespace-nowrap transition-colors border-l-2 cursor-pointer ${
                  i === activeIdx
                    ? "text-[var(--accent-primary)] bg-[var(--surface-hover)] border-l-[var(--accent-primary)]"
                    : "text-[var(--text-muted)] hover:text-[var(--text-primary)] border-l-transparent"
                }`}
                onClick={() => handleSelect(i)}
              >
                <span>t{tab.id + 1}</span>
                {i === activeIdx && tabs.length > 1 && (
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDeleteTab(i);
                    }}
                    className="p-0.5 rounded hover:bg-red-500/20 text-[var(--text-muted)] hover:text-red-400 transition-colors"
                    title="Delete terminal"
                  >
                    <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                      />
                    </svg>
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
