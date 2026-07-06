import { useMemo, useRef, useState, useCallback, useEffect, type ReactNode } from "react";
import { useLineHover } from "../../hooks/useLineHover";
import { CommentBadge } from "../Comment/CommentBadge";
import { CommentEditor } from "../Comment/CommentEditor";
import { CommentView } from "../Comment/CommentView";
import { subscribeToComments, getCommentsByFile, addComment } from "../../stores/comments";
import type { LineComment } from "../../types/events";

const LARGE_DIFF_THRESHOLD = 500;

type DiffStyle = "unified" | "split";

interface DiffViewerProps {
  diffText: string;
  filePath?: string;
  baseUrl?: string;
  onAsk?: ((prompt: string) => void) | null;
  className?: string;
  mode?: DiffStyle;
  visible?: boolean;
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

export function DiffViewer({ diffText, filePath, baseUrl, onAsk, className, mode = "unified", visible = true }: DiffViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const { hoveredLine } = useLineHover(containerRef);

  const [editingLine, setEditingLine] = useState<number | null>(null);
  const [sendToAI, setSendToAI] = useState(false);
  const [fileComments, setFileComments] = useState<LineComment[]>([]);
  const [forceRender, setForceRender] = useState(false);

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

  const totalChanged = useMemo(() => {
    let adds = 0;
    let dels = 0;
    for (const line of lines) {
      if (line.type === "add") adds++;
      else if (line.type === "remove") dels++;
    }
    return adds + dels;
  }, [lines]);

  const handleBadgeClick = useCallback((line: number) => {
    setEditingLine((prev) => (prev === line ? null : line));
    setSendToAI(false);
  }, []);

  const handleEditorSubmit = useCallback((comment: string) => {
    if (!filePath || editingLine == null) return;
    addComment({ file: filePath, line: editingLine, comment, resolved: false });
    if (sendToAI && onAsk) {
      onAsk(`User comment on file \`${filePath}\` line ${editingLine}:\n\n${comment}\n\nPlease analyze this line of code and suggest how to address it.`);
    }
    setEditingLine(null);
  }, [filePath, editingLine, sendToAI, onAsk]);

  const handleEditorCancel = useCallback(() => {
    setEditingLine(null);
  }, []);

  // Virtual placeholder
  if (!visible) {
    return <div className="diff-viewer-placeholder" style={{ height: "160px" }} />;
  }

  if (!diffText) {
    return <div className="text-sm text-neutral-500 p-4">No diff content</div>;
  }

  // Large file warning
  if (!forceRender && totalChanged > LARGE_DIFF_THRESHOLD) {
    return (
      <div data-slot="review-large-diff" className="px-3 py-4 text-center border border-dashed border-[var(--border-base)] rounded mx-2 my-1">
        <div className="text-xs font-medium text-[var(--text-muted)] mb-1">
          Large diff: {totalChanged.toLocaleString()} changed lines (limit: {LARGE_DIFF_THRESHOLD.toLocaleString()})
        </div>
        <button
          onClick={() => setForceRender(true)}
          className="px-3 py-1 text-xs rounded border border-[var(--border-base)] text-[var(--text-primary)] hover:bg-[var(--surface-hover)]"
        >
          Render anyway
        </button>
      </div>
    );
  }

  // ── Split mode ──
  if (mode === "split") {
    return renderSplitDiff(lines, filePath, baseUrl, onAsk, containerRef, hoveredLine, commentsByLine, editingLine, sendToAI, setSendToAI, handleBadgeClick, handleEditorSubmit, handleEditorCancel);
  }

  // ── Unified mode ──
  return renderUnifiedDiff(lines, filePath, baseUrl, onAsk, containerRef, hoveredLine, commentsByLine, editingLine, sendToAI, setSendToAI, handleBadgeClick, handleEditorSubmit, handleEditorCancel, className);
}

function renderUnifiedDiff(
  lines: DiffLine[],
  filePath: string | undefined,
  baseUrl: string | undefined,
  onAsk: ((prompt: string) => void) | null | undefined,
  containerRef: React.RefObject<HTMLDivElement | null>,
  hoveredLine: number | null,
  commentsByLine: Map<number, LineComment[]>,
  editingLine: number | null,
  sendToAI: boolean,
  setSendToAI: (v: boolean) => void,
  handleBadgeClick: (line: number) => void,
  handleEditorSubmit: (comment: string) => void,
  handleEditorCancel: () => void,
  className?: string,
) {
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
        <div className={`diff-line ${typeClass} relative cursor-pointer`} data-line={displayLineNum} onClick={() => filePath && handleBadgeClick(displayLineNum)}>
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
              <CommentView key={c.id} comment={c} inline onSendToAgent={onAsk ? (cmt) => onAsk(`User comment on file \`${cmt.file}\` line ${cmt.line}:\n\n${cmt.comment}\n\nPlease analyze this line of code and suggest how to address it.`) : undefined} />
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

function renderSplitDiff(
  lines: DiffLine[],
  filePath: string | undefined,
  baseUrl: string | undefined,
  onAsk: ((prompt: string) => void) | null | undefined,
  containerRef: React.RefObject<HTMLDivElement | null>,
  hoveredLine: number | null,
  commentsByLine: Map<number, LineComment[]>,
  editingLine: number | null,
  sendToAI: boolean,
  setSendToAI: (v: boolean) => void,
  handleBadgeClick: (line: number) => void,
  handleEditorSubmit: (comment: string) => void,
  handleEditorCancel: () => void,
) {
  const leftLines: ReactNode[] = [];
  const rightLines: ReactNode[] = [];
  let oldLineNum = 0;
  let newLineNum = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    if (line.type === "header") {
      leftLines.push(
        <div key={i} className="diff-line diff-line-header"><span className="diff-line-content">{line.content}</span></div>
      );
      rightLines.push(
        <div key={i} className="diff-line diff-line-header"><span className="diff-line-content">{line.content}</span></div>
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

    const displayLineNum = line.type === "remove" ? oldLineNum : newLineNum;
    const isHoveredLine = filePath && hoveredLine === displayLineNum;
    const isEditing = filePath && editingLine === displayLineNum;
    const lineComments = filePath ? (commentsByLine.get(displayLineNum) || []) : [];

    // Left column (deletions + context)
    if (line.type === "context" || line.type === "remove") {
      const typeClass = line.type === "remove" ? "diff-line-remove" : "";
      leftLines.push(
        <div key={i} className={`diff-line ${typeClass}`}>
          <span className="diff-line-number">{String(oldLineNum)}</span>
          <span className="diff-line-content">{line.content}</span>
        </div>
      );
    } else {
      leftLines.push(<div key={i} className="diff-line diff-line-empty">&nbsp;</div>);
    }

    // Right column (additions + context)
    if (line.type === "context" || line.type === "add") {
      const typeClass = line.type === "add" ? "diff-line-add" : "";
      rightLines.push(
        <div key={i} className={`diff-line ${typeClass} relative`}>
          <span className="diff-line-number relative">
            {filePath && (line.type === "add" || line.type === "context") && (
              <CommentBadge
                line={displayLineNum}
                hasComments={lineComments.length > 0}
                isHovered={!!isHoveredLine}
                onClick={() => handleBadgeClick(displayLineNum)}
              />
            )}
            {String(newLineNum)}
          </span>
          <span className="diff-line-content">{line.content}</span>
        </div>
      );
    } else {
      rightLines.push(<div key={i} className="diff-line diff-line-empty">&nbsp;</div>);
    }
  }

  return (
    <div ref={containerRef} className="diff-viewer-split">
      <div className="diff-split-left">{leftLines}</div>
      <div className="diff-split-divider" />
      <div className="diff-split-right">
        {rightLines}
        {editingLine != null && filePath && (
          <CommentEditor
            file={filePath}
            line={editingLine}
            sendToAI={sendToAI}
            onSendToAIToggle={setSendToAI}
            onSubmit={handleEditorSubmit}
            onCancel={handleEditorCancel}
            baseUrl={baseUrl}
          />
        )}
      </div>
    </div>
  );
}
