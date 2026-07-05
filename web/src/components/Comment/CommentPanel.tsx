import { useState, useEffect, useSyncExternalStore } from "react";
import { MessageSquare, X, Loader2 } from "lucide-react";
import { subscribeToComments, getCommentsSnapshot, getAllComments, syncCommentsFromServer } from "../../stores/comments";
import { CommentView } from "./CommentView";
import type { LineComment } from "../../types/events";

interface CommentPanelProps {
  baseUrl?: string;
  sessionId?: string | null;
  onAsk?: ((prompt: string) => void) | null;
  onClose?: () => void;
}

export function CommentPanel({ baseUrl, sessionId, onAsk, onClose }: CommentPanelProps) {
  const [loadingServer, setLoadingServer] = useState(false);

  useEffect(() => {
    if (!baseUrl || !sessionId) return;
    setLoadingServer(true);
    fetch(`${baseUrl}/comments/${sessionId}`)
      .then((r) => r.ok ? r.json() : null)
      .then((data) => {
        if (data?.comments) {
          syncCommentsFromServer(data.comments);
        }
        setLoadingServer(false);
      })
      .catch(() => setLoadingServer(false));
  }, [baseUrl, sessionId]);

  const comments = useSyncExternalStore(subscribeToComments, getCommentsSnapshot, getCommentsSnapshot);
  const hasResolved = comments.some((c) => c.resolved);
  const [showResolved, setShowResolved] = useState(false);

  const filtered = showResolved ? comments : comments.filter((c) => !c.resolved);
  const sorted = [...filtered].sort((a, b) => b.time - a.time);

  const handleSendToAgent = (comment: LineComment) => {
    if (!onAsk) return;
    const prompt = `用户对文件 \`${comment.file}\` 第 ${comment.line} 行的评论:\n\n${comment.comment}\n\n请分析该行代码并给出处理建议。`;
    onAsk(prompt);
  };

  const handleSyncToServer = async () => {
    if (!baseUrl || !sessionId) return;
    setLoadingServer(true);
    try {
      await fetch(`${baseUrl}/comments/${sessionId}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ comments }),
      });
    } catch {}
    setLoadingServer(false);
  };

  return (
    <div className="flex flex-col h-full bg-[var(--surface-base)]">
      <div className="flex items-center justify-between px-3 py-1.5 border-b border-[var(--border-base)] bg-[var(--surface-secondary)] shrink-0">
        <div className="flex items-center gap-2">
          <MessageSquare className="w-3.5 h-3.5 text-[var(--text-muted)]" />
          <span className="text-xs font-medium">Comments</span>
          <span className="text-[10px] text-[var(--text-muted)]">({comments.length})</span>
        </div>
        <div className="flex items-center gap-1">
          {baseUrl && sessionId && (
            <button
              onClick={handleSyncToServer}
              disabled={loadingServer}
              className="p-0.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
              title="Sync to server"
            >
              {loadingServer
                ? <Loader2 className="w-3 h-3 animate-spin" />
                : <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                  </svg>
              }
            </button>
          )}
          {onClose && (
            <button
              onClick={onClose}
              className="p-0.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)] hover:text-[var(--text-primary)]"
            >
              <X className="w-3 h-3" />
            </button>
          )}
        </div>
      </div>

      {hasResolved && (
        <div className="px-3 py-1 border-b border-[var(--border-base)] shrink-0">
          <label className="flex items-center gap-1.5 text-[10px] text-[var(--text-muted)] cursor-pointer">
            <input
              type="checkbox"
              checked={showResolved}
              onChange={() => setShowResolved((v) => !v)}
              className="w-3 h-3 rounded border-[var(--border-base)] accent-[var(--accent-primary)]"
            />
            Show resolved
          </label>
        </div>
      )}

      <div className="flex-1 overflow-auto">
        {sorted.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-[var(--text-muted)] gap-2 py-8">
            <MessageSquare className="w-6 h-6" />
            <span className="text-xs">No comments yet</span>
            <span className="text-[10px]">Hover line numbers in file preview to add comments</span>
          </div>
        ) : (
          sorted.map((comment) => (
            <CommentView
              key={comment.id}
              comment={comment}
              onSendToAgent={onAsk ? handleSendToAgent : undefined}
            />
          ))
        )}
      </div>
    </div>
  );
}
