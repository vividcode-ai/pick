import { useState, useRef, useEffect, useCallback } from "react";
import { Send, Loader2 } from "lucide-react";

interface MentionItem {
  path: string;
  name: string;
}

interface CommentEditorProps {
  file: string;
  line: number;
  initialValue?: string;
  sendToAI?: boolean;
  onSendToAIToggle?: (v: boolean) => void;
  onSubmit: (comment: string) => void;
  onCancel: () => void;
  baseUrl?: string;
  embedded?: boolean;
}

export function CommentEditor({
  file,
  line,
  initialValue = "",
  sendToAI = false,
  onSendToAIToggle,
  onSubmit,
  onCancel,
  baseUrl,
  embedded,
}: CommentEditorProps) {
  const [text, setText] = useState(initialValue);
  const [mentionOpen, setMentionOpen] = useState(false);
  const [mentionQuery, setMentionQuery] = useState("");
  const [mentionItems, setMentionItems] = useState<MentionItem[]>([]);
  const [mentionIdx, setMentionIdx] = useState(0);
  const [searching, setSearching] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleInput = useCallback((value: string) => {
    setText(value);
    const atIdx = value.lastIndexOf("@");
    if (atIdx >= 0 && (atIdx === 0 || value[atIdx - 1] === " ")) {
      const query = value.slice(atIdx + 1);
      setMentionQuery(query);
      if (baseUrl && query.trim()) {
        setSearching(true);
        fetch(`${baseUrl}/find/files?pattern=${encodeURIComponent(query)}&limit=10`)
          .then((r) => r.ok ? r.json() : { files: [] })
          .then((data) => {
            const files = (data.files || data.matches || data.results || []).map((f: any) => {
              const path = f.path || f.file || f.name || "";
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
  }, [baseUrl]);

  const insertMention = useCallback((item: MentionItem) => {
    const atIdx = text.lastIndexOf("@");
    if (atIdx < 0) return;
    const before = text.slice(0, atIdx);
    const after = text.slice(atIdx + mentionQuery.length + 1);
    const newText = `${before}@${item.path} ${after}`;
    setText(newText);
    setMentionOpen(false);
    setMentionItems([]);
    textareaRef.current?.focus();
  }, [text, mentionQuery]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
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
        return;
      }
    }

    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handleSubmit();
      return;
    }
    if (e.key === "Escape" && !mentionOpen) {
      e.preventDefault();
      onCancel();
    }
  }, [mentionOpen, mentionItems, mentionIdx, insertMention, onCancel]);

  const handleSubmit = useCallback(() => {
    const trimmed = text.trim();
    if (!trimmed) return;
    onSubmit(trimmed);
    setText("");
  }, [text, onSubmit]);

  return (
    <div className={embedded ? "" : "border border-[var(--border-base)] rounded-lg bg-[var(--surface-base)] shadow-md mx-8 my-1"}>
      {!embedded && (
        <div className="flex items-center justify-between px-3 py-1.5 border-b border-[var(--border-base)]">
          <span className="text-[10px] text-[var(--text-muted)]">
            {file.split("/").pop() || file}:{line}
          </span>
          <span className="text-[10px] text-[var(--text-muted)]">Comment</span>
        </div>
      )}

      <div className="relative">
        <textarea
          ref={textareaRef}
          value={text}
          onChange={(e) => handleInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Write a comment... (@ to reference files)"
          rows={2}
          className="w-full resize-none px-3 py-1.5 text-xs bg-transparent text-[var(--text-primary)] outline-none placeholder-[var(--text-muted)] font-mono"
          style={{ fieldSizing: "content" }}
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
                <span className="text-[11px]">📄</span>
                <span className="truncate">{item.path}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="flex items-center justify-between px-3 py-1.5 border-t border-[var(--border-base)]">
        <label className="flex items-center gap-1.5 text-[10px] text-[var(--text-muted)] cursor-pointer">
          <input
            type="checkbox"
            checked={sendToAI}
            onChange={() => onSendToAIToggle?.(!sendToAI)}
            className="w-3 h-3 rounded border-[var(--border-base)] accent-[var(--accent-primary)]"
          />
          <Send className="w-3 h-3" />
          Send to AI
        </label>

        <div className="flex items-center gap-2">
          <button
            onClick={onCancel}
            className="px-2 py-0.5 text-[10px] rounded text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
          >
            Cancel
          </button>
          <button
            onClick={handleSubmit}
            disabled={!text.trim()}
            className="px-3 py-0.5 text-[10px] font-medium rounded bg-[var(--accent-primary)] text-white hover:opacity-90 disabled:opacity-40"
          >
            Comment
          </button>
        </div>
      </div>
    </div>
  );
}
