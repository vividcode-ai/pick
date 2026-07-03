import { useCallback, useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

const MIN_HEIGHT = 100;
const MAX_HEIGHT_FRACTION = 0.6;

interface TerminalPanelProps {
  baseUrl: string;
  visible: boolean;
  onClose: () => void;
}

export function TerminalPanel({ baseUrl, visible, onClose }: TerminalPanelProps) {
  const [panelHeight, setPanelHeight] = useState(200);
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const initializedRef = useRef(false);
  const draggingRef = useRef(false);

  useEffect(() => {
    if (!visible || !containerRef.current || initializedRef.current) return;
    initializedRef.current = true;

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
    fitAddonRef.current = fitAddon;

    // Open terminal immediately (not dependent on WebSocket)
    term.open(containerRef.current!);
    setTimeout(() => fitAddon.fit(), 50);

    const hostname = (() => {
      try {
        return new URL(baseUrl).hostname;
      } catch {
        return "127.0.0.1";
      }
    })();

    (async () => {
      let wsPort = 9000;
      try {
        const res = await fetch(`${baseUrl}/server-config`);
        if (res.ok) {
          const cfg = await res.json();
          wsPort = cfg.pty_ws_port ?? 9000;
        }
      } catch {}

      const ws = new WebSocket(`ws://${hostname}:${wsPort}`);
      wsRef.current = ws;

      const decodeBuffer = (buf: ArrayBuffer): string => {
        try {
          const utf8 = new TextDecoder("utf-8", { fatal: true });
          return utf8.decode(buf);
        } catch {
          const gbk = new TextDecoder("gbk");
          return gbk.decode(buf);
        }
      };

      ws.onmessage = (event) => {
        if (event.data instanceof Blob) {
          event.data.arrayBuffer().then((buf) => {
            term.write(decodeBuffer(buf).replace(/\n/g, "\r\n"));
          });
        } else {
          term.write(event.data.replace(/\n/g, "\r\n"));
        }
      };

      ws.onclose = () => {
        term.write("\r\n\x1b[31m[Connection closed]\x1b[0m\r\n");
      };

      ws.onerror = () => {
        term.write("\r\n\x1b[31m[Connection error]\x1b[0m\r\n");
      };

      term.onData((data) => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(data);
        }
      });

      term.onResize(({ cols, rows }) => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ type: "resize", cols, rows }));
        }
      });
    })();

    termRef.current = term;

    const handleResize = () => {
      fitAddonRef.current?.fit();
    };
    window.addEventListener("resize", handleResize);

    return () => {
      window.removeEventListener("resize", handleResize);
      wsRef.current?.close();
      term.dispose();
      initializedRef.current = false;
    };
  }, [visible, baseUrl]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    draggingRef.current = true;
    const startY = e.clientY;
    const startHeight = panelHeight;

    const handleMouseMove = (ev: MouseEvent) => {
      if (!draggingRef.current) return;
      const delta = startY - ev.clientY;
      const newHeight = Math.max(MIN_HEIGHT, Math.min(window.innerHeight * MAX_HEIGHT_FRACTION, startHeight + delta));
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
  }, [panelHeight]);

  return (
    <div
      className="border-t border-[var(--border-base)] bg-[#1e1e2e]"
      style={{ display: visible ? "block" : "none" }}
    >
      <div className="flex items-center justify-between px-3 py-1 bg-[#181825] border-b border-[var(--border-base)]">
        <span className="text-xs text-[var(--text-muted)] font-medium">Terminal</span>
        <button
          onClick={onClose}
          className="p-0.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
        >
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div
        onMouseDown={handleMouseDown}
        className="h-[5px] cursor-row-resize bg-transparent hover:bg-[var(--accent-primary)] transition-colors"
      />
      <div ref={containerRef} style={{ height: panelHeight }} />
    </div>
  );
}
