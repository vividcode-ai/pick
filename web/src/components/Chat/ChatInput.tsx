import {
  useState,
  useRef,
  useEffect,
  useCallback,
  type KeyboardEvent,
} from "react";
import { ChevronDown, Loader2 } from "lucide-react";
import fuzzysort from "fuzzysort";
import type { ProviderInfo } from "../../types/events";
import { ModelThinkingSelector } from "./ModelThinkingSelector";
import { CommandMode } from "./CommandMode";
import { GoalDrawer } from "./GoalDrawer";

interface MentionItem {
  path: string;
  name: string;
}

interface ChatInputProps {
  onSend: (text: string, opts?: { mode?: "build" | "plan"; extraMode?: "goal" | "loop" | null }) => void;
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
  activeGoal: { objective: string; startTime: number } | null;
  onClearGoal: () => void;
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
  activeGoal,
  onClearGoal,
}: ChatInputProps) {
  const [input, setInput] = useState("");
  const [currentCommand, setCurrentCommand] = useState<"build" | "plan">(
    "build",
  );
  const [commandOpen, setCommandOpen] = useState(false);
  const [extraMode, setExtraMode] = useState<"goal" | "loop" | null>(null);
  const [browsingHistory, setBrowsingHistory] = useState(false);
  const [mentionOpen, setMentionOpen] = useState(false);
  const [mentionQuery, setMentionQuery] = useState("");
  const [mentionItems, setMentionItems] = useState<MentionItem[]>([]);
  const [mentionIdx, setMentionIdx] = useState(0);
  const [searching, setSearching] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const commandRef = useRef<HTMLDivElement>(null);
  const popupRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    textareaRef.current?.focus();
  }, [connected, sessionId]);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (
        commandRef.current &&
        !commandRef.current.contains(e.target as Node)
      ) {
        setCommandOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  useEffect(() => {
    if (!popupRef.current || mentionIdx < 0) return;
    const child = popupRef.current.children[mentionIdx] as HTMLElement;
    if (child) {
      child.scrollIntoView({ block: "nearest" });
    }
  }, [mentionIdx]);

  async function navigateHistory(
    direction: "up" | "down",
  ): Promise<{ text: string | null; browsing: boolean }> {
    try {
      const res = await fetch(`${baseUrl}/prompt-history/navigate`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ direction, current_input: input }),
      });
      if (!res.ok) return { text: null, browsing: false };
      const data = await res.json();
      return { text: data.text ?? null, browsing: data.browsing ?? false };
    } catch {
      return { text: null, browsing: false };
    }
  }

  async function pushHistory(text: string) {
    try {
      await fetch(`${baseUrl}/prompt-history/push`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ text }),
      });
    } catch {
      /* ignore */
    }
  }

  const handleSend = () => {
    const trimmed = input.trim();
    if (!trimmed) return;
    onSend(trimmed, { mode: currentCommand, extraMode });
    pushHistory(trimmed);
    setInput("");
    setExtraMode(null);
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
        selectMention(mentionItems[mentionIdx]);
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

  const handleInputChange = useCallback(
    (value: string) => {
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
        setSearching(true);
        const lastSep = query.lastIndexOf("/");
        const dirPath = lastSep >= 0 ? query.slice(0, lastSep) || "." : ".";
        const fileFilter = lastSep >= 0 ? query.slice(lastSep + 1) : query;
        const listPath = dirPath === "." ? "." : dirPath;
        fetch(
          `${baseUrl}/files/list?path=${encodeURIComponent(listPath)}&limit=50`,
        )
          .then((r) => (r.ok ? r.json() : { entries: [] }))
          .then((data) => {
            let entries: MentionItem[] = (data.entries || []).map((e: any) => {
              const prefix = dirPath === "." ? "" : dirPath + "/";
              return {
                path: prefix + e.name + (e["type"] === "directory" ? "/" : ""),
                name: e.name,
              };
            });
            if (fileFilter) {
              entries = fuzzysort
                .go(fileFilter, entries, { key: "name", threshold: -1000 })
                .map((r) => r.obj);
            }
            if (fileFilter && entries.some((e) => e.path === query)) {
              setMentionItems([]);
              setMentionOpen(false);
              setSearching(false);
              return;
            }
            setMentionItems(entries);
            setMentionOpen(entries.length > 0);
            setMentionIdx(0);
            setSearching(false);
          })
          .catch(() => setSearching(false));
      } else {
        setMentionOpen(false);
        setMentionItems([]);
      }
    },
    [baseUrl, handleAutoResize],
  );

  const selectMention = useCallback(
    (item: MentionItem) => {
      if (item.path.endsWith("/")) {
        const el = textareaRef.current;
        if (!el) return;
        const cursor = el.selectionStart;
        const beforeCursor = input.slice(0, cursor);
        const atIdx = beforeCursor.lastIndexOf("@");
        if (atIdx < 0) return;
        const queryEnd = atIdx + 1 + mentionQuery.length;
        const before = input.slice(0, atIdx);
        const after = input.slice(queryEnd).replace(/^\s+/, "");
        const newInput = `${before}@${item.path}${after}`;
        setInput(newInput);
        setMentionQuery(item.path);
        setSearching(true);
        const dir = item.path.replace(/\/$/, "");
        fetch(`${baseUrl}/files/list?path=${encodeURIComponent(dir)}&limit=50`)
          .then((r) => (r.ok ? r.json() : { entries: [] }))
          .then((data) => {
            const entries: MentionItem[] = (data.entries || []).map(
              (e: any) => ({
                path:
                  item.path + e.name + (e["type"] === "directory" ? "/" : ""),
                name: e.name,
              }),
            );
            setMentionItems(entries);
            setMentionOpen(entries.length > 0);
            setMentionIdx(0);
            setSearching(false);
          })
          .catch(() => setSearching(false));
        textareaRef.current?.focus();
        setTimeout(() => handleAutoResize(), 0);
      } else {
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
      }
    },
    [input, mentionQuery, handleAutoResize, baseUrl],
  );

  const executeCommand = (cmd: "build" | "plan") => {
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
        {/* Top bar: working status + goal on the same line */}
        {(streaming || activeGoal) && (
          <div className="flex items-center gap-3 pb-3 px-1">
            {streaming && (
              <div className="flex items-center gap-2 text-[var(--text-muted)] shrink-0 whitespace-nowrap">
                <span className="w-2 h-2 bg-[var(--text-muted)] rounded-full animate-pulse" />
                <span className="text-sm">Working...</span>
              </div>
            )}
            {activeGoal && (
              <div className={streaming ? "min-w-0 flex-1" : "w-full"}>
                <GoalDrawer
                  goal={activeGoal}
                  onEdit={(newObjective) => {}}
                  onPause={() => {}}
                  onDelete={onClearGoal}
                  noWrapper={!!streaming}
                />
              </div>
            )}
            {/* Invisible spacer: mirrors Working width so Goal is centered */}
            {streaming && (
              <div
                className="flex items-center gap-2 text-[var(--text-muted)] shrink-0 whitespace-nowrap invisible"
                aria-hidden="true"
              >
                <span className="w-2 h-2 bg-[var(--text-muted)] rounded-full" />
                <span className="text-sm">Working...</span>
              </div>
            )}
          </div>
        )}
        {!streaming && pendingMessages.length > 0 && (
          <div className="flex flex-col gap-1 px-1 pb-3">
            {pendingMessages.map((msg, i) => (
              <div
                key={i}
                className="flex items-start gap-2 text-[var(--text-muted)] text-xs bg-[var(--surface-elevated)]/60 rounded-lg px-3 py-1.5"
              >
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
              <div
                ref={popupRef}
                className="absolute bottom-full left-2 right-2 mb-1 bg-[var(--surface-elevated)] border border-[var(--border-base)] rounded-md shadow-lg max-h-[160px] overflow-auto z-10"
              >
                {searching && (
                  <div className="flex items-center gap-2 px-3 py-1.5 text-xs text-[var(--text-muted)]">
                    <Loader2 className="w-3 h-3 animate-spin" />
                    Searching...
                  </div>
                )}
                {!searching &&
                  mentionItems.map((item, i) => (
                    <div
                      key={item.path}
                      className={`flex items-center gap-2 px-3 py-1 text-xs cursor-pointer ${
                        i === mentionIdx
                          ? "bg-[var(--surface-hover)] text-[var(--text-primary)]"
                          : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
                      }`}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        selectMention(item);
                      }}
                      onMouseEnter={() => setMentionIdx(i)}
                    >
                      <span className="text-[var(--text-muted)] shrink-0">
                        <svg
                          className="w-3 h-3"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="2"
                        >
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
          <div className="flex items-center px-3 pb-3 pt-1.5">
            {/* Left: extra mode icon + command dropdown */}
            <div className="flex-1 flex items-center justify-start">
              <CommandMode value={extraMode} onChange={setExtraMode} disabled={disabled} connected={connected} />

              <div className="relative flex items-center" ref={commandRef}>
                <button
                  onClick={() => setCommandOpen((v) => !v)}
                  disabled={disabled || !connected}
                  onKeyDown={handleCommandKeyDown}
                  className="inline-flex items-center gap-1 cursor-pointer text-xs text-[var(--text-primary)] hover:bg-[var(--surface-hover)] rounded-md px-1.5 py-0.5"
                >
                  {currentCommand.charAt(0).toUpperCase() +
                    currentCommand.slice(1)}
                  <ChevronDown className="w-3 h-3 text-[var(--text-muted)]" />
                </button>
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
            </div>

            {/* Right: model selectors + send */}
            <div className="flex items-center gap-2">
              <ModelThinkingSelector
                providers={providers}
                selectedModel={selectedModel}
                selectedProvider={selectedProvider}
                onModelChange={onModelChange}
                thinkingLevel={thinkingLevel}
                onThinkingLevelChange={onThinkingLevelChange}
                disabled={disabled || !connected}
                baseUrl={baseUrl}
                onProvidersChange={onProvidersChange}
                hiddenModels={hiddenModels}
                onToggleHidden={onToggleHidden}
                onEnsureVisible={onEnsureVisible}
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
                  <svg
                    className="w-4 h-4 text-[var(--text-primary)]"
                    viewBox="0 0 16 16"
                    fill="currentColor"
                  >
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
