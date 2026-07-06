import { useMemo, useRef, useState, useCallback, useEffect, type ReactNode } from "react";
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

  const lines = useMemo(() => parseDiff(diffText), [diffText]);

  const handleBadgeClick = useCallback((line: number) => {
    setEditingLine((prev) => (prev === line ? null : line));
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
    const isHoveredLine = filePath && hoveredLine === displayLineNum;
    const isEditing = filePath && editingLine === displayLineNum;
    const lineComments = filePath ? (commentsByLine.get(displayLineNum) || []) : [];

    lineEls.push(
      <div key={i}>
        <div className={`diff-line ${typeClass} relative cursor-pointer`} onClick={() => filePath && handleBadgeClick(displayLineNum)}>
          <span className="diff-line-number">
            {line.type === "remove" || line.type === "context" ? String(oldLineNum) : ""}
          </span>
          <span className="diff-line-number relative">
            {filePath && (
              <CommentBadge
                line={displayLineNum}
                hasComments={lineComments.length > 0}
                isHovered={!!isHoveredLine}
                onClick={() => handleBadgeClick(displayLineNum)}
              />
            )}
            {line.type === "add" || line.type === "context" ? String(newLineNum) : ""}
          </span>
          <span className="diff-line-content">{line.content}</span>
        </div>
        {lineComments.length > 0 && !isEditing && (
          <div className="ml-8 border-l-2 border-[var(--accent-primary)]/30">
            {lineComments.map((c) => (
              <CommentView key={c.id} comment={c} inline onSendToAgent={onAsk ? (cmt) => onAsk(`用户对文件 \`${cmt.file}\` 第 ${cmt.line} 行的评论:\n\n${cmt.comment}\n\n请分析该行代码并给出处理建议。`) : undefined} />
            ))}
          </div>
        )}
        {isEditing && filePath && (
          <CommentEditor
            file={filePath}
            line={displayLineNum}
            sendToAI={sendToAI}
            onSendToAIToggle={setSendToAI}
            onSubmit={handleEditorSubmit}
            onCancel={handleEditorCancel}
            baseUrl={baseUrl}
          />
        )}
      </div>
    );
  }

  return (
    <div ref={containerRef} className={`diff-viewer ${className ?? ""}`}>
      {lineEls}
    </div>
  );
}
