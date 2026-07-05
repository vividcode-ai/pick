import { useState, useEffect, useRef } from "react";
import { File, Loader2, AlertCircle } from "lucide-react";
import { highlightCode } from "../../lib/highlight";

interface FilePreviewProps {
  baseUrl: string;
  filePath: string | null;
}

export function FilePreview({ baseUrl, filePath }: FilePreviewProps) {
  const [content, setContent] = useState<string | null>(null);
  const [html, setHtml] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [totalLines, setTotalLines] = useState<number>(0);
  const activePathRef = useRef<string | null>(null);

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
      <div
        className="flex-1 overflow-auto [&_pre]:!m-0 [&_pre]:!h-full [&_pre]:!rounded-none [&_pre]:!bg-transparent [&_.line-num]:inline-block [&_.line-num]:w-[3rem] [&_.line-num]:text-right [&_.line-num]:pr-3 [&_.line-num]:mr-3 [&_.line-num]:text-[var(--text-muted)] [&_.line-num]:select-none [&_.line-num]:border-r [&_.line-num]:border-[var(--border-base)] [&_.line-num]:text-[11px]"
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </div>
  );
}
