import { useMemo, useRef, useState, useCallback, type ReactNode } from "react";
import { useLineHover } from "../../hooks/useLineHover";
import { CommentBadge } from "../Comment/CommentBadge";
import { CommentEditor } from "../Comment/CommentEditor";
import { CommentView } from "../Comment/CommentView";
import { subscribeToComments, getCommentsByFile, addComment } from "../../stores/comments";
import type { LineComment } from "../../types/events";

interface DiffViewerProps {
  diffText: string;
  filePath?: string;
  baseUrl?: string;
  onAsk?: ((prompt: string) => void) | null;
  className?: string;
}

interface DiffLine {
  type: "add" | "remove" | "header" | "context";
  oldNum?: string;
  newNum?: string;
  content: string;
}

function parseDiff(diffText: string): DiffLine[] {
  const lines = diffText.split("\n");
  const result: DiffLine[] = [];

  for (const line of lines) {
    if (line.startsWith("diff --git") || line.startsWith("index ") || line.startsWith("--- ") || line.startsWith("+++ ")) {
      result.push({ type: "header", content: line });
    } else if (line.startsWith("@@")) {
      result.push({ type: "header", content: line });
    } else if (line.startsWith("+")) {
      result.push({ type: "add", content: line });
    } else if (line.startsWith("-")) {
      result.push({ type: "remove", content: line });
    } else {
      result.push({ type: "context", content: line });
    }
  }

  return result;
}

export function DiffViewer({ diffText, filePath, baseUrl, onAsk, className }: DiffViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const { hoveredLine } = useLineHover(containerRef);

  const [editorOpen, setEditorOpen] = useState(false);
  const [editorLine, setEditorLine] = useState<number | null>(null);
  const [sendToAI, setSendToAI] = useState(false);
  const [showComments, setShowComments] = useState<number | null>(null);

  const fileComments = filePath ? getCommentsByFile(filePath) : [];
  const commentsByLine = new Map<number, LineComment[]>();
  for (const c of fileComments) {
    const list = commentsByLine.get(c.line) || [];
    list.push(c);
    commentsByLine.set(c.line, list);
  }

  const lines = useMemo(() => parseDiff(diffText), [diffText]);

  const handleBadgeClick = useCallback((line: number) => {
    const comments = commentsByLine.get(line);
    if (comments && comments.length > 0) {
      setShowComments(showComments === line ? null : line);
    } else {
      setEditorLine(line);
      setEditorOpen(true);
      setSendToAI(false);
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

  if (!diffText) {
    return <div className="text-sm text-neutral-500 p-4">No diff content</div>;
  }

  const lineEls: ReactNode[] = [];
  let oldLineNum = 0;
  let newLineNum = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    if (line.type === "header") {
      lineEls.push(
        <div key={i} className="diff-line diff-line-header">
          <span className="diff-line-content">{line.content}</span>
        </div>
      );
      continue;
    }

    if (line.type === "add") {
      newLineNum++;
    } else if (line.type === "remove") {
      oldLineNum++;
    } else {
      oldLineNum++;
      newLineNum++;
    }

    const typeClass = line.type === "add" ? "diff-line-add" : line.type === "remove" ? "diff-line-remove" : "";
    const displayLineNum = line.type === "remove" ? oldLineNum : newLineNum;
    const hasComments = filePath && commentsByLine.has(displayLineNum);
    const isHoveredLine = filePath && hoveredLine === displayLineNum;
    const showLineComments = filePath && showComments === displayLineNum;

    lineEls.push(
      <div key={i} className={`diff-line ${typeClass} relative`}>
        <span className="diff-line-number">
          {line.type === "remove" || line.type === "context" ? String(oldLineNum) : ""}
        </span>
        <span className="diff-line-number relative">
          {filePath && (
            <CommentBadge
              line={displayLineNum}
              hasComments={!!hasComments}
              isHovered={!!isHoveredLine}
              onClick={() => handleBadgeClick(displayLineNum)}
            />
          )}
          {line.type === "add" || line.type === "context" ? String(newLineNum) : ""}
        </span>
        <span className="diff-line-content">{line.content}</span>
        {showLineComments && hasComments && (
          <div className="absolute left-0 right-0 top-full z-10 bg-[var(--surface-base)] border border-[var(--border-base)] rounded shadow-lg">
            {commentsByLine.get(displayLineNum)?.map((c) => (
              <CommentView key={c.id} comment={c} onSendToAgent={onAsk ? (cmt) => onAsk(`用户对文件 \`${cmt.file}\` 第 ${cmt.line} 行的评论:\n\n${cmt.comment}\n\n请分析该行代码并给出处理建议。`) : undefined} />
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <div ref={containerRef} className={`diff-viewer ${className ?? ""}`}>
      {lineEls}

      {editorOpen && editorLine != null && filePath && (
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
