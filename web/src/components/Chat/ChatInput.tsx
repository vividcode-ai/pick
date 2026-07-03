import { useState, useRef, useEffect, type KeyboardEvent } from "react";
import type { ProviderInfo } from "../../types/events";
import { ModelSelector } from "./ModelSelector";
import { ThinkingSelector } from "./ThinkingSelector";

interface ChatInputProps {
  onSend: (text: string) => void;
  disabled: boolean;
  onCancel?: () => void;
  connected: boolean;
  streaming: boolean;
  providers: ProviderInfo[];
  selectedModel: string;
  selectedProvider?: string;
  onModelChange: (modelId: string, provider: string) => void;
  thinkingLevel: string;
  onThinkingLevelChange: (l: string) => void;
  sessionId?: string | null;
  pendingMessages: string[];
  baseUrl: string;
}

export function ChatInput({
  onSend,
  disabled,
  onCancel,
  connected,
  streaming,
  providers,
  selectedModel,
  selectedProvider,
  onModelChange,
  thinkingLevel,
  onThinkingLevelChange,
  sessionId,
  pendingMessages,
  baseUrl,
}: ChatInputProps) {
  const [input, setInput] = useState("");
  const [currentCommand, setCurrentCommand] = useState<"build" | "plan">("build");
  const [commandOpen, setCommandOpen] = useState(false);
  const [browsingHistory, setBrowsingHistory] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const commandRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    textareaRef.current?.focus();
  }, [connected, sessionId]);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (commandRef.current && !commandRef.current.contains(e.target as Node)) {
        setCommandOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  async function navigateHistory(direction: "up" | "down"): Promise<{ text: string | null; browsing: boolean }> {
    try {
      const res = await fetch(`${baseUrl}/prompt-history/navigate`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ direction, current_input: input }),
      });
      if (!res.ok) return { text: null, browsing: false };
      const data = await res.json();
      return { text: data.text ?? null, browsing: data.browsing ?? false };
    } catch { return { text: null, browsing: false }; }
  }

  async function pushHistory(text: string) {
    try {
      await fetch(`${baseUrl}/prompt-history/push`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ text }),
      });
    } catch { /* ignore */ }
  }

  const handleSend = () => {
    const trimmed = input.trim();
    if (!trimmed) return;
    onSend(trimmed);
    pushHistory(trimmed);
    setInput("");
    setBrowsingHistory(false);
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
  };

  const handleKeyDown = async (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
      return;
    }
    if (e.key === "Escape" && disabled && onCancel) {
      onCancel();
      return;
    }
    if (e.key === "ArrowUp") {
      if (!connected || !baseUrl) return;
      e.preventDefault();
      const result = await navigateHistory("up");
      if (result.text !== null) {
        setInput(result.text);
        setBrowsingHistory(result.browsing);
      }
      return;
    }
    if (e.key === "ArrowDown") {
      if (!connected || !baseUrl) return;
      e.preventDefault();
      const result = await navigateHistory("down");
      if (result.text !== null) {
        setInput(result.text);
        setBrowsingHistory(result.browsing);
      }
      return;
    }
    // Any other key exits history browsing
    if (browsingHistory) {
      setBrowsingHistory(false);
    }
  };

  const handleInput = () => {
    const el = textareaRef.current;
    if (el) {
      el.style.height = "auto";
      el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
    }
  };

  const insertCommand = (cmd: string) => {
    const newVal = cmd + " ";
    setInput(newVal);
    setBrowsingHistory(false);
    if (textareaRef.current) {
      textareaRef.current.focus();
      const len = newVal.length;
      textareaRef.current.setSelectionRange(len, len);
      handleInput();
    }
  };

  const executeCommand = (cmd: "build" | "plan") => {
    insertCommand(`/${cmd}`);
    setCurrentCommand(cmd);
    setCommandOpen(false);
  };

  const handleCommandKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Tab") {
      e.preventDefault();
      setCurrentCommand((prev) => (prev === "build" ? "plan" : "build"));
    }
    if (e.key === "Escape") {
      setCommandOpen(false);
    }
  };

  return (
      <div className="w-full px-4 py-3">
      <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[40%] mx-auto">
        {streaming && (
          <div className="flex items-center gap-2 text-neutral-400 px-1 pb-3">
            <span className="w-2 h-2 bg-neutral-400 rounded-full animate-pulse" />
            <span className="text-sm">Working...</span>
          </div>
        )}
        {pendingMessages.length > 0 && (
          <div className="flex flex-col gap-1 px-1 pb-3">
            {pendingMessages.map((msg, i) => (
              <div key={i} className="flex items-start gap-2 text-neutral-400 text-xs bg-neutral-800/60 rounded-lg px-3 py-1.5">
                <span className="w-1.5 h-1.5 bg-neutral-500 rounded-full mt-1 shrink-0" />
                <span className="line-clamp-2">{msg}</span>
              </div>
            ))}
          </div>
        )}
        <div className="rounded-2xl border border-neutral-700 bg-neutral-800">
        {/* Top: textarea */}
        <textarea
          ref={textareaRef}
          autoFocus
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          onInput={handleInput}
          placeholder={connected ? "Type a message..." : "Connecting..."}
          rows={1}
          disabled={!connected}
          className="w-full bg-transparent text-neutral-100 px-4 pt-3 pb-3 text-sm resize-none outline-none placeholder-neutral-500 disabled:opacity-50 min-h-[44px]"
        />

        {/* Bottom: controls */}
        <div className="flex items-center justify-between px-3 pb-3 pt-1.5 border-t border-neutral-700/50">
          {/* Left: command dropdown */}
          <div className="relative" ref={commandRef}>
            <div className="flex">
              <button
                onClick={() => insertCommand(`/${currentCommand}`)}
                disabled={disabled || !connected}
                onKeyDown={handleCommandKeyDown}
                className="px-2.5 py-1 text-xs rounded-l-md bg-neutral-700 hover:bg-neutral-600 text-neutral-300 disabled:opacity-40 transition-colors"
              >
                {currentCommand.charAt(0).toUpperCase() + currentCommand.slice(1)}
              </button>
              <button
                onClick={() => setCommandOpen((v) => !v)}
                disabled={disabled || !connected}
                className="px-1 py-1 text-xs rounded-r-md bg-neutral-700 hover:bg-neutral-600 text-neutral-400 border-l border-neutral-600 disabled:opacity-40 transition-colors"
              >
                <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                </svg>
              </button>
            </div>
            {commandOpen && (
              <div className="absolute bottom-full left-0 mb-1 w-24 rounded-md bg-neutral-800 border border-neutral-700 shadow-lg z-50 overflow-hidden">
                <button
                  onClick={() => executeCommand("build")}
                  className="w-full px-3 py-1.5 text-xs text-left text-neutral-300 hover:bg-neutral-700 transition-colors"
                >
                  Build
                </button>
                <button
                  onClick={() => executeCommand("plan")}
                  className="w-full px-3 py-1.5 text-xs text-left text-neutral-300 hover:bg-neutral-700 transition-colors"
                >
                  Plan
                </button>
              </div>
            )}
          </div>

          {/* Right: model selectors + send */}
          <div className="flex items-center gap-2">
            <ModelSelector
              providers={providers}
              selectedModel={selectedModel}
              selectedProvider={selectedProvider}
              onModelChange={onModelChange}
              disabled={disabled || !connected}
            />

            <ThinkingSelector
              providers={providers}
              selectedModel={selectedModel}
              thinkingLevel={thinkingLevel}
              onThinkingLevelChange={onThinkingLevelChange}
              disabled={disabled}
            />

            <button
              onClick={streaming ? onCancel : handleSend}
              disabled={!streaming && (!input.trim() || !connected)}
              className="p-1.5 rounded-lg bg-neutral-700 hover:bg-neutral-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              title={streaming ? "Stop" : "Send"}
            >
              {streaming ? (
                <svg className="w-4 h-4" viewBox="0 0 16 16" fill="#ef4444">
                  <rect x="2" y="2" width="12" height="12" rx="1.5" />
                </svg>
              ) : (
                <svg className="w-4 h-4 text-white" viewBox="0 0 16 16" fill="currentColor">
                  <path d="M8 2l6 6h-4v6H6V8H2z" />
                </svg>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
    </div>
  );
}
