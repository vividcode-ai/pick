import { useState, useEffect, useRef, useCallback } from "react";
import { File, Loader2, AlertCircle, MessageSquare } from "lucide-react";
import { highlightCodeLines } from "../../lib/highlight";
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
  const [lineHtmls, setLineHtmls] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [totalLines, setTotalLines] = useState<number>(0);
  const activePathRef = useRef<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [editingLine, setEditingLine] = useState<number | null>(null);
  const [sendToAI, setSendToAI] = useState(false);
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
      setLineHtmls([]);
      setError(null);
      return;
    }

    activePathRef.current = filePath;
    setLoading(true);
    setError(null);
    setLineHtmls([]);

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
          const lines = await highlightCodeLines(data.content, filePath);
          if (activePathRef.current === filePath) {
            setLineHtmls(lines);
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

  const handleLineClick = useCallback((lineNum: number) => {
    setEditingLine((prev) => (prev === lineNum ? null : lineNum));
    setSendToAI(false);
  }, []);

  const handleEditorSubmit = useCallback((comment: string) => {
    if (!filePath || editingLine == null) return;
    addComment({ file: filePath, line: editingLine, comment, resolved: false });
    if (sendToAI && onAsk) {
      onAsk(`用户对文件 \`${filePath}\` 第 ${editingLine} 行的评论:\n\n${comment}\n\n请分析该行代码并给出处理建议。`);
    }
    setEditingLine(null);
  }, [filePath, editingLine, sendToAI, onAsk]);

  const handleEditorCancel = useCallback(() => {
    setEditingLine(null);
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
          className="absolute inset-0 overflow-auto font-mono text-xs leading-tight"
        >
          {lineHtmls.map((innerHtml, idx) => {
            const lineNum = idx + 1;
            const lineComments = commentsByLine.get(lineNum) || [];
            const isEditing = editingLine === lineNum;
            return (
              <div key={lineNum}>
                <div
                  className="group line-wrapper relative flex hover:bg-[var(--surface-hover)]/30 cursor-pointer select-none"
                  onClick={() => handleLineClick(lineNum)}
                >
                  <span
                    className="line-num shrink-0 w-[3rem] text-right pr-3 text-[var(--text-muted)] select-none border-r border-[var(--border-base)] text-[11px] leading-[18px]"
                  >
                    {lineNum}
                  </span>
                  <span
                    className="line flex-1 leading-[18px] pl-3 select-text"
                    dangerouslySetInnerHTML={{ __html: innerHtml }}
                  />
                  <span className="shrink-0 w-5 flex items-center justify-center opacity-0 group-hover:opacity-100 text-[var(--text-muted)] transition-opacity">
                    <MessageSquare className="w-3 h-3" />
                  </span>
                </div>
                {lineComments.length > 0 && !isEditing && (
                  <div className="border-l-2 border-[var(--accent-primary)]/30 ml-[3rem]">
                    {lineComments.map((c) => (
                      <CommentView key={c.id} comment={c} inline onSendToAgent={onAsk ? (cmt) => onAsk(`用户对文件 \`${cmt.file}\` 第 ${cmt.line} 行的评论:\n\n${cmt.comment}\n\n请分析该行代码并给出处理建议。`) : undefined} />
                    ))}
                  </div>
                )}
                {isEditing && (
                  <CommentEditor
                    file={filePath}
                    line={lineNum}
                    sendToAI={sendToAI}
                    onSendToAIToggle={setSendToAI}
                    onSubmit={handleEditorSubmit}
                    onCancel={handleEditorCancel}
                    baseUrl={baseUrl}
                  />
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
