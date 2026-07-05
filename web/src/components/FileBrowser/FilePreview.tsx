import { useState, useEffect, useRef, useCallback, useSyncExternalStore } from "react";
import { File, Loader2, AlertCircle } from "lucide-react";
import { highlightCode } from "../../lib/highlight";
import { useLineHover } from "../../hooks/useLineHover";
import { CommentBadge } from "../Comment/CommentBadge";
import { CommentEditor } from "../Comment/CommentEditor";
import { subscribeToComments, getCommentsByFile, addComment } from "../../stores/comments";
import type { LineComment } from "../../types/events";

interface FilePreviewProps {
  baseUrl: string;
  filePath: string | null;
  onAsk?: ((prompt: string) => void) | null;
}

export function FilePreview({ baseUrl, filePath, onAsk }: FilePreviewProps) {
  const [content, setContent] = useState<string | null>(null);
  const [html, setHtml] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [totalLines, setTotalLines] = useState<number>(0);
  const activePathRef = useRef<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editorLine, setEditorLine] = useState<number | null>(null);
  const [sendToAI, setSendToAI] = useState(false);

  const { hoveredLine, hoveredRect } = useLineHover(containerRef);

  const fileComments = useSyncExternalStore(
    subscribeToComments,
    () => filePath ? getCommentsByFile(filePath) : [],
    () => filePath ? getCommentsByFile(filePath) : [],
  );

  const commentsByLine = new Map<number, LineComment[]>();
  for (const c of fileComments) {
    const list = commentsByLine.get(c.line) || [];
    list.push(c);
    commentsByLine.set(c.line, list);
  }

  useEffect(() => {
    if (!filePath) {
      setContent(null);
      setHtml("");
      setError(null);
      return;
    }

    activePathRef.current = filePath;
    setLoading(true);
    setError(null);
    setContent(null);
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
          setContent(data.content);
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

  const handleBadgeClick = useCallback((line: number) => {
    setEditorLine(line);
    setEditorOpen(true);
    setSendToAI(false);
  }, []);

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
          className="absolute inset-0 overflow-auto [&_pre]:!m-0 [&_pre]:!min-h-full [&_pre]:!rounded-none [&_pre]:!bg-transparent [&_pre]:relative [&_.line-num]:inline-block [&_.line-num]:w-[3rem] [&_.line-num]:text-right [&_.line-num]:pr-3 [&_.line-num]:mr-3 [&_.line-num]:text-[var(--text-muted)] [&_.line-num]:select-none [&_.line-num]:border-r [&_.line-num]:border-[var(--border-base)] [&_.line-num]:text-[11px] [&_.line-num]:relative [&_.shiki]:!bg-transparent"
          dangerouslySetInnerHTML={{ __html: html }}
        />
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
