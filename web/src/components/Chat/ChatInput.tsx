import { useState, useRef, useEffect, useCallback, type KeyboardEvent } from "react";
import { Loader2 } from "lucide-react";
import type { ProviderInfo } from "../../types/events";
import { ModelSelector } from "./ModelSelector";
import { ThinkingSelector } from "./ThinkingSelector";

interface MentionItem {
  path: string;
  name: string;
}

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
  onProvidersChange?: () => void;
  hiddenModels: string[];
  onToggleHidden: (key: string) => void;
  onEnsureVisible: (key: string) => void;
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
  onProvidersChange,
  hiddenModels,
  onToggleHidden,
  onEnsureVisible,
}: ChatInputProps) {
  const [input, setInput] = useState("");
  const [currentCommand, setCurrentCommand] = useState<"build" | "plan">("build");
  const [commandOpen, setCommandOpen] = useState(false);
  const [browsingHistory, setBrowsingHistory] = useState(false);
  const [mentionOpen, setMentionOpen] = useState(false);
  const [mentionQuery, setMentionQuery] = useState("");
  const [mentionItems, setMentionItems] = useState<MentionItem[]>([]);
  const [mentionIdx, setMentionIdx] = useState(0);
  const [searching, setSearching] = useState(false);
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
    setMentionOpen(false);
    setMentionItems([]);
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
  };

  const handleKeyDown = async (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (mentionOpen && mentionItems.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setMentionIdx((i) => Math.min(i + 1, mentionItems.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setMentionIdx((i) => Math.max(i - 1, 0));
        return;
      }
      if (e.key === "Tab" || (e.key === "Enter" && mentionItems.length > 0)) {
        e.preventDefault();
        insertMention(mentionItems[mentionIdx]);
        return;
      }
      if (e.key === "Escape") {
        setMentionOpen(false);
        setMentionItems([]);
        return;
      }
    }
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

  const handleAutoResize = useCallback(() => {
    const el = textareaRef.current;
    if (el) {
      el.style.height = "auto";
      el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
    }
  }, []);

  const handleInputChange = useCallback((value: string) => {
    setInput(value);
    handleAutoResize();
    const el = textareaRef.current;
    if (!el) return;
    const cursor = el.selectionStart;
    const beforeCursor = value.slice(0, cursor);
    const atIdx = beforeCursor.lastIndexOf("@");
    if (atIdx >= 0 && (atIdx === 0 || beforeCursor[atIdx - 1] === " ")) {
      const afterAt = beforeCursor.slice(atIdx + 1);
      const spaceIdx = afterAt.indexOf(" ");
      const query = spaceIdx >= 0 ? afterAt.slice(0, spaceIdx) : afterAt;
      setMentionQuery(query);
      if (query.length >= 2) {
        setSearching(true);
        fetch(`${baseUrl}/find/files?pattern=${encodeURIComponent(query)}&limit=10&prefix=true`)
          .then((r) => r.ok ? r.json() : { files: [] })
          .then((data) => {
            const files: MentionItem[] = (data.files || []).map((f: any) => {
              const path: string = f.path || "";
              const parts = path.replace(/\\/g, "/").split("/");
              return { path, name: parts[parts.length - 1] || path };
            });
            setMentionItems(files);
            setMentionOpen(files.length > 0);
            setMentionIdx(0);
            setSearching(false);
          })
          .catch(() => setSearching(false));
      } else {
        setMentionOpen(false);
        setMentionItems([]);
      }
    } else {
      setMentionOpen(false);
      setMentionItems([]);
    }
  }, [baseUrl, handleAutoResize]);

  const insertMention = useCallback((item: MentionItem) => {
    const el = textareaRef.current;
    if (!el) return;
    const cursor = el.selectionStart;
    const beforeCursor = input.slice(0, cursor);
    const atIdx = beforeCursor.lastIndexOf("@");
    if (atIdx < 0) return;
    const queryEnd = atIdx + 1 + mentionQuery.length;
    const before = input.slice(0, atIdx);
    const after = input.slice(queryEnd).replace(/^\s+/, "");
    const newInput = `${before}@${item.path} ${after}`;
    setInput(newInput);
    setMentionOpen(false);
    setMentionItems([]);
    textareaRef.current?.focus();
    setTimeout(() => handleAutoResize(), 0);
  }, [input, mentionQuery, handleAutoResize]);

  const insertCommand = (cmd: string) => {
    const newVal = cmd + " ";
    setInput(newVal);
    setBrowsingHistory(false);
    if (textareaRef.current) {
      textareaRef.current.focus();
      const len = newVal.length;
      textareaRef.current.setSelectionRange(len, len);
      handleAutoResize();
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
          <div className="flex items-center gap-2 text-[var(--text-muted)] px-1 pb-3">
            <span className="w-2 h-2 bg-[var(--text-muted)] rounded-full animate-pulse" />
            <span className="text-sm">Working...</span>
          </div>
        )}
        {pendingMessages.length > 0 && (
          <div className="flex flex-col gap-1 px-1 pb-3">
            {pendingMessages.map((msg, i) => (
              <div key={i} className="flex items-start gap-2 text-[var(--text-muted)] text-xs bg-[var(--surface-elevated)]/60 rounded-lg px-3 py-1.5">
                <span className="w-1.5 h-1.5 bg-[var(--text-muted)] rounded-full mt-1 shrink-0" />
                <span className="line-clamp-2">{msg}</span>
              </div>
            ))}
          </div>
        )}
        <div className="rounded-2xl border border-[var(--border-base)] bg-[var(--surface-base)]">
        {/* Top: textarea */}
        <div className="relative">
        <textarea
          ref={textareaRef}
          autoFocus
          value={input}
          onChange={(e) => handleInputChange(e.target.value)}
          onKeyDown={handleKeyDown}
          onInput={handleAutoResize}
          placeholder={connected ? "Type a message..." : "Connecting..."}
          rows={1}
          disabled={!connected}
          className="w-full bg-transparent text-[var(--text-primary)] px-4 pt-3 pb-3 text-sm resize-none outline-none placeholder-[var(--text-muted)] disabled:opacity-50 min-h-[44px]"
        />
        {mentionOpen && mentionItems.length > 0 && (
          <div className="absolute bottom-full left-2 right-2 mb-1 bg-[var(--surface-elevated)] border border-[var(--border-base)] rounded-md shadow-lg max-h-[160px] overflow-auto z-10">
            {searching && (
              <div className="flex items-center gap-2 px-3 py-1.5 text-xs text-[var(--text-muted)]">
                <Loader2 className="w-3 h-3 animate-spin" />
                Searching...
              </div>
            )}
            {!searching && mentionItems.map((item, i) => (
              <div
                key={item.path}
                className={`flex items-center gap-2 px-3 py-1 text-xs cursor-pointer ${
                  i === mentionIdx
                    ? "bg-[var(--surface-hover)] text-[var(--text-primary)]"
                    : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
                }`}
                onMouseDown={(e) => { e.preventDefault(); insertMention(item); }}
                onMouseEnter={() => setMentionIdx(i)}
              >
                <span className="text-[var(--text-muted)] shrink-0">
                  <svg className="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                    <polyline points="14 2 14 8 20 8" />
                    <line x1="16" y1="13" x2="8" y2="13" />
                    <line x1="16" y1="17" x2="8" y2="17" />
                    <polyline points="10 9 9 9 8 9" />
                  </svg>
                </span>
                <span className="truncate">{item.path}</span>
              </div>
            ))}
          </div>
        )}
        </div>

        {/* Bottom: controls */}
        <div className="flex items-center justify-between px-3 pb-3 pt-1.5 border-t border-[var(--border-base)]/50">
          {/* Left: command dropdown */}
          <div className="relative" ref={commandRef}>
            <div className="flex">
              <button
                onClick={() => insertCommand(`/${currentCommand}`)}
                disabled={disabled || !connected}
                onKeyDown={handleCommandKeyDown}
                className="px-2.5 py-1 text-xs rounded-l-md bg-[var(--surface-button)] hover:opacity-80 text-[var(--text-primary)] disabled:opacity-40 transition-colors"
              >
                {currentCommand.charAt(0).toUpperCase() + currentCommand.slice(1)}
              </button>
              <button
                onClick={() => setCommandOpen((v) => !v)}
                disabled={disabled || !connected}
                className="px-1 py-1 text-xs rounded-r-md bg-[var(--surface-button)] hover:opacity-80 text-[var(--text-muted)] border-l border-[var(--border-base)] disabled:opacity-40 transition-colors"
              >
                <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                </svg>
              </button>
            </div>
            {commandOpen && (
              <div className="absolute bottom-full left-0 mb-1 w-24 rounded-md bg-[var(--surface-elevated)] border border-[var(--border-base)] shadow-lg z-50 overflow-hidden">
                <button
                  onClick={() => executeCommand("build")}
                  className="w-full px-3 py-1.5 text-xs text-left text-[var(--text-primary)] hover:bg-[var(--surface-hover)] transition-colors"
                >
                  Build
                </button>
                <button
                  onClick={() => executeCommand("plan")}
                  className="w-full px-3 py-1.5 text-xs text-left text-[var(--text-primary)] hover:bg-[var(--surface-hover)] transition-colors"
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
              baseUrl={baseUrl}
              onProvidersChange={onProvidersChange}
              hiddenModels={hiddenModels}
              onToggleHidden={onToggleHidden}
              onEnsureVisible={onEnsureVisible}
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
              className="p-1.5 rounded-lg bg-[var(--surface-button)] hover:opacity-80 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              title={streaming ? "Stop" : "Send"}
            >
              {streaming ? (
                <svg className="w-4 h-4" viewBox="0 0 16 16" fill="#ef4444">
                  <rect x="2" y="2" width="12" height="12" rx="1.5" />
                </svg>
              ) : (
                <svg className="w-4 h-4 text-[var(--text-primary)]" viewBox="0 0 16 16" fill="currentColor">
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
