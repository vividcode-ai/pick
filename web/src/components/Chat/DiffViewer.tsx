import { useMemo, type ReactNode } from "react";

interface DiffViewerProps {
  diffText: string;
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

export function DiffViewer({ diffText, className }: DiffViewerProps) {
  const lines = useMemo(() => parseDiff(diffText), [diffText]);

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

    lineEls.push(
      <div key={i} className={`diff-line ${typeClass}`}>
        <span className="diff-line-number">
          {line.type === "remove" || line.type === "context" ? String(oldLineNum) : ""}
        </span>
        <span className="diff-line-number">
          {line.type === "add" || line.type === "context" ? String(newLineNum) : ""}
        </span>
        <span className="diff-line-content">{line.content}</span>
      </div>
    );
  }

  return (
    <div className={`diff-viewer ${className ?? ""}`}>
      {lineEls}
    </div>
  );
}
