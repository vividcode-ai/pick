import { useState, useEffect, useRef, useCallback } from "react";
import { File, Loader2, AlertCircle } from "lucide-react";
import { highlightCode } from "../../lib/highlight";
import { CommentEditor } from "../Comment/CommentEditor";
import { CommentView } from "../Comment/CommentView";
import { subscribeToComments, getCommentsByFile, addComment } from "../../stores/comments";
import type { LineComment } from "../../types/events";

interface FilePreviewProps {
  baseUrl: string;
  filePath: string | null;
  onAsk?: ((prompt: string) => void) | null;
}

export function FilePreview({ baseUrl, filePath, onAsk }: FilePreviewProps) {
  const [html, setHtml] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [totalLines, setTotalLines] = useState<number>(0);
  const activePathRef = useRef<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editorLine, setEditorLine] = useState<number | null>(null);
  const [sendToAI, setSendToAI] = useState(false);
  const [showComments, setShowComments] = useState<number | null>(null);
  const [fileComments, setFileComments] = useState<LineComment[]>([]);

  useEffect(() => {
    if (!filePath) {
      setFileComments([]);
      return;
    }
    setFileComments(getCommentsByFile(filePath));
    const unsub = subscribeToComments(() => {
      setFileComments(getCommentsByFile(filePath));
    });
    return () => { unsub(); };
  }, [filePath]);

  const commentsByLine = new Map<number, LineComment[]>();
  for (const c of fileComments) {
    const list = commentsByLine.get(c.line) || [];
    list.push(c);
    commentsByLine.set(c.line, list);
  }

  useEffect(() => {
    if (!filePath) {
      setHtml("");
      setError(null);
      return;
    }

    activePathRef.current = filePath;
    setLoading(true);
    setError(null);
    setHtml("");

    fetch(`${baseUrl}/files/content?path=${encodeURIComponent(filePath)}`)
      .then(async (res) => {
        if (!res.ok) {
          const text = await res.text();
          throw new Error(text || `HTTP ${res.status}`);
        }
        return res.json();
      })
      .then(async (data) => {
        if (activePathRef.current !== filePath) return;
        if (data.binary) {
          setError("Binary file - cannot preview");
        } else {
          setTotalLines(data.total_lines ?? 0);
          const highlighted = await highlightCode(data.content, filePath);
          if (activePathRef.current === filePath) {
            setHtml(highlighted);
          }
        }
        setLoading(false);
      })
      .catch((e) => {
        if (activePathRef.current === filePath) {
          setError(e.message || "Failed to load file");
          setLoading(false);
        }
      });
  }, [baseUrl, filePath]);

  const handleContainerClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    const btn = target.closest(".line-add-btn") as HTMLElement | null;
    if (btn) {
      const wrapper = btn.closest(".line-wrapper") as HTMLElement | null;
      if (wrapper?.dataset.line) {
        const line = parseInt(wrapper.dataset.line, 10);
        const existing = commentsByLine.get(line);
        if (existing && existing.length > 0) {
          setShowComments(showComments === line ? null : line);
        } else {
          setEditorLine(line);
          setEditorOpen(true);
          setSendToAI(false);
        }
      }
      return;
    }

    const commentDot = target.closest(".line-comment-dot") as HTMLElement | null;
    if (commentDot) {
      const wrapper = commentDot.closest(".line-wrapper") as HTMLElement | null;
      if (wrapper?.dataset.line) {
        const line = parseInt(wrapper.dataset.line, 10);
        setShowComments(showComments === line ? null : line);
      }
    }
  }, [commentsByLine, showComments]);

  const handleEditorSubmit = useCallback((comment: string) => {
    if (!filePath || editorLine == null) return;
    addComment({ file: filePath, line: editorLine, comment, resolved: false });
    if (sendToAI && onAsk) {
      onAsk(`用户对文件 \`${filePath}\` 第 ${editorLine} 行的评论:\n\n${comment}\n\n请分析该行代码并给出处理建议。`);
    }
    setEditorOpen(false);
    setEditorLine(null);
  }, [filePath, editorLine, sendToAI, onAsk]);

  const handleEditorCancel = useCallback(() => {
    setEditorOpen(false);
    setEditorLine(null);
  }, []);

  if (!filePath) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-[var(--text-muted)] gap-2">
        <File className="w-8 h-8" />
        <span className="text-xs">Select a file to preview</span>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 className="w-5 h-5 animate-spin text-[var(--text-muted)]" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-[var(--text-muted)] gap-2">
        <AlertCircle className="w-5 h-5 text-red-400" />
        <span className="text-xs">{error}</span>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <div className="text-xs text-[var(--text-muted)] px-4 py-1.5 border-b border-[var(--border-base)] shrink-0">
        {filePath}
        <span className="ml-2">— {totalLines} lines</span>
      </div>
      <div className="flex-1 min-h-0 relative">
        <div
          ref={containerRef}
          onClick={handleContainerClick}
          className="absolute inset-0 overflow-auto leading-tight
            [&_pre]:!m-0 [&_pre]:!min-h-full [&_pre]:!rounded-none [&_pre]:!bg-transparent [&_pre]:!p-0
            [&_.line-num]:inline-block [&_.line-num]:w-[3rem] [&_.line-num]:text-right [&_.line-num]:pr-3 [&_.line-num]:mr-3
            [&_.line-num]:text-[var(--text-muted)] [&_.line-num]:select-none
            [&_.line-num]:border-r [&_.line-num]:border-[var(--border-base)] [&_.line-num]:text-[11px]
            [&_.line-wrapper]:relative [&_.line-wrapper]:leading-tight [&_.line-wrapper]:hover:bg-[var(--surface-hover)]/30
            [&_.line-add-btn]:hidden [&_.line-wrapper:hover_.line-add-btn]:inline-flex
            [&_.line-add-btn]:absolute [&_.line-add-btn]:left-[2px] [&_.line-add-btn]:top-0
            [&_.line-add-btn]:items-center [&_.line-add-btn]:justify-center
            [&_.line-add-btn]:w-4 [&_.line-add-btn]:h-full [&_.line-add-btn]:z-10
            [&_.line-add-btn]:text-[var(--text-muted)] [&_.line-add-btn]:hover:text-[var(--accent-primary)]
            [&_.line-add-btn]:cursor-pointer [&_.line-add-btn]:bg-transparent [&_.line-add-btn]:border-none
            [&_.line-comment-dot]:hidden [&_.line-wrapper:hover_.line-comment-dot]:inline-flex
            [&_.line-comment-dot]:w-4 [&_.line-comment-dot]:h-full [&_.line-comment-dot]:items-center [&_.line-comment-dot]:justify-center
            [&_.line-comment-dot]:text-blue-400 [&_.line-comment-dot]:cursor-pointer"
          dangerouslySetInnerHTML={{ __html: html }}
        />

        {showComments != null && commentsByLine.get(showComments) && (
          <div
            className="absolute right-0 top-0 w-[300px] max-h-full overflow-auto bg-[var(--surface-base)] border-l border-[var(--border-base)] shadow-lg z-20"
            onMouseDown={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between px-3 py-1.5 border-b border-[var(--border-base)] text-xs font-medium">
              <span>Comments (line {showComments})</span>
              <button
                onClick={() => setShowComments(null)}
                className="p-0.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)]"
              >
                <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
            {commentsByLine.get(showComments)?.map((c) => (
              <CommentView key={c.id} comment={c} onSendToAgent={onAsk ? (cmt) => onAsk(`用户对文件 \`${cmt.file}\` 第 ${cmt.line} 行的评论:\n\n${cmt.comment}\n\n请分析该行代码并给出处理建议。`) : undefined} />
            ))}
            <div className="px-3 py-2 border-t border-[var(--border-base)]">
              <button
                onClick={() => { setEditorLine(showComments); setEditorOpen(true); setSendToAI(false); setShowComments(null); }}
                className="text-xs text-[var(--accent-primary)] hover:underline"
              >
                + Add comment
              </button>
            </div>
          </div>
        )}
      </div>

      {editorOpen && editorLine != null && (
        <>
          <div className="fixed inset-0 z-[2199] bg-black/30" onClick={handleEditorCancel} />
          <CommentEditor
            file={filePath}
            line={editorLine}
            sendToAI={sendToAI}
            onSendToAIToggle={setSendToAI}
            onSubmit={handleEditorSubmit}
            onCancel={handleEditorCancel}
            baseUrl={baseUrl}
          />
        </>
      )}
    </div>
  );
}
