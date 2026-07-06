import { useState } from "react";
import { Trash2, CheckCircle, RotateCw, Send, MessageSquare } from "lucide-react";
import type { LineComment } from "../../types/events";
import { removeComment, resolveComment, unresolveComment } from "../../stores/comments";

interface CommentViewProps {
  comment: LineComment;
  onSendToAgent?: (comment: LineComment) => void;
  inline?: boolean;
}

export function CommentView({ comment, onSendToAgent, inline }: CommentViewProps) {
  const [deleting, setDeleting] = useState(false);

  const handleDelete = () => {
    removeComment(comment.id);
  };

  const handleToggleResolve = () => {
    if (comment.resolved) {
      unresolveComment(comment.id);
    } else {
      resolveComment(comment.id);
    }
  };

  const timeStr = new Date(comment.time).toLocaleString(undefined, {
    month: "short", day: "numeric", hour: "2-digit", minute: "2-digit",
  });

  if (inline) {
    return (
      <div className={`mx-8 my-1 px-3 py-1.5 border border-[var(--border-base)] rounded-lg text-xs transition-colors ${
        comment.resolved ? "opacity-50" : "bg-[var(--surface-base)]"
      }`}>
        <div className="flex items-center justify-between">
          <span className="text-[10px] text-[var(--text-muted)]">{timeStr}</span>
          {comment.resolved && <span className="text-[10px] text-green-400">Resolved</span>}
        </div>
        <div className={`whitespace-pre-wrap leading-relaxed mt-0.5 ${comment.resolved ? "line-through" : ""}`}>
          {comment.comment}
        </div>
        <div className="flex items-center gap-2 mt-1">
          <button onClick={handleToggleResolve} className="text-[10px] text-[var(--text-muted)] hover:text-[var(--text-primary)]" title={comment.resolved ? "Reopen" : "Resolve"}>
            {comment.resolved ? <RotateCw className="w-3 h-3" /> : <CheckCircle className="w-3 h-3" />}
          </button>
          {onSendToAgent && (
            <button onClick={() => onSendToAgent(comment)} className="text-[10px] text-[var(--text-muted)] hover:text-[var(--text-primary)]" title="Ask AI">
              <Send className="w-3 h-3" />
            </button>
          )}
          <button onClick={handleDelete} className="text-[10px] text-red-400 hover:text-red-300" title="Delete">
            <Trash2 className="w-3 h-3" />
          </button>
          <button onClick={() => setDeleting(false)} className="text-[10px] text-[var(--text-muted)] hover:text-[var(--text-primary)]">
            <MessageSquare className="w-3 h-3" /> Reply
          </button>
        </div>
      </div>
    );
  }

  return (
    <div
      className={`group relative px-3 py-2 border-b border-[var(--border-base)] text-xs transition-colors ${
        comment.resolved ? "opacity-50" : ""
      }`}
    >
      <div className="flex items-center justify-between mb-1">
        <span className="text-[10px] text-[var(--text-muted)]">
          <span className="font-medium text-[var(--text-primary)]">{comment.file.split("/").pop() || comment.file}</span>
          <span className="mx-1">:</span>
          <span className="font-mono">{comment.line}</span>
          <span className="mx-1">·</span>
          {timeStr}
        </span>
        {comment.resolved && (
          <span className="text-[10px] text-green-400">Resolved</span>
        )}
      </div>

      <div className={`whitespace-pre-wrap leading-relaxed ${comment.resolved ? "line-through" : ""}`}>
        {comment.comment}
      </div>

      <div className="flex items-center gap-1 mt-1.5 opacity-0 group-hover:opacity-100 transition-opacity">
        <button
          onClick={handleToggleResolve}
          className="flex items-center gap-1 px-1.5 py-0.5 text-[10px] rounded text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
          title={comment.resolved ? "Reopen" : "Resolve"}
        >
          {comment.resolved
            ? <RotateCw className="w-3 h-3" />
            : <CheckCircle className="w-3 h-3" />
          }
          {comment.resolved ? "Reopen" : "Resolve"}
        </button>

        {onSendToAgent && (
          <button
            onClick={() => onSendToAgent(comment)}
            className="flex items-center gap-1 px-1.5 py-0.5 text-[10px] rounded text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
            title="Send to AI agent"
          >
            <Send className="w-3 h-3" />
            Ask AI
          </button>
        )}

        <button
          onClick={handleDelete}
          className="flex items-center gap-1 px-1.5 py-0.5 text-[10px] rounded text-red-400 hover:bg-red-500/20"
          title="Delete comment"
        >
          <Trash2 className="w-3 h-3" />
          Delete
        </button>
      </div>
    </div>
  );
}
