import { useState, useEffect } from "react";
import { File, Loader2, AlertCircle } from "lucide-react";

interface FilePreviewProps {
  baseUrl: string;
  filePath: string | null;
}

export function FilePreview({ baseUrl, filePath }: FilePreviewProps) {
  const [content, setContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [totalLines, setTotalLines] = useState<number>(0);

  useEffect(() => {
    if (!filePath) {
      setContent(null);
      setError(null);
      return;
    }

    setLoading(true);
    setError(null);
    setContent(null);

    fetch(`${baseUrl}/files/content?path=${encodeURIComponent(filePath)}`)
      .then(async (res) => {
        if (!res.ok) {
          const text = await res.text();
          throw new Error(text || `HTTP ${res.status}`);
        }
        return res.json();
      })
      .then((data) => {
        if (data.binary) {
          setError("Binary file - cannot preview");
        } else {
          setContent(data.content);
          setTotalLines(data.total_lines ?? 0);
        }
        setLoading(false);
      })
      .catch((e) => {
        setError(e.message || "Failed to load file");
        setLoading(false);
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

  const ext = filePath.split(".").pop()?.toLowerCase() || "";
  const isMarkdown = ext === "md";

  if (isMarkdown) {
    return (
      <div className="h-full overflow-auto p-4">
        <div className="text-xs text-[var(--text-muted)] mb-2">{filePath}</div>
        <pre className="text-sm whitespace-pre-wrap font-sans leading-relaxed">{content}</pre>
      </div>
    );
  }

  const lines = content?.split("\n") || [];

  return (
    <div className="h-full flex flex-col">
      <div className="text-xs text-[var(--text-muted)] px-4 py-1.5 border-b border-[var(--border-base)] shrink-0">
        {filePath}
        <span className="ml-2">— {totalLines} lines</span>
      </div>
      <div className="flex-1 overflow-auto">
        <table className="w-full border-collapse font-mono text-xs leading-relaxed">
          <tbody>
            {lines.map((line, i) => (
              <tr key={i} className="hover:bg-[var(--surface-hover)]/50">
                <td className="text-right text-[var(--text-muted)] px-3 select-none w-12 border-r border-[var(--border-base)] align-top">
                  {i + 1}
                </td>
                <td className="px-3 whitespace-pre">{line}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
