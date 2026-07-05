import { useState, useEffect } from "react";
import { File, Loader2, AlertCircle } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypePrettyCode from "rehype-pretty-code";

const EXT_TO_LANG: Record<string, string> = {
  ts: "typescript", tsx: "tsx", js: "javascript", jsx: "jsx",
  rs: "rust", py: "python", go: "go", zig: "zig",
  css: "css", scss: "scss", less: "less",
  json: "json", md: "markdown", html: "html", xml: "xml",
  yaml: "yaml", yml: "yaml", toml: "toml",
  sh: "bash", bash: "bash", zsh: "bash", ps1: "powershell",
  sql: "sql", graphql: "graphql",
  dockerfile: "dockerfile", makefile: "makefile",
  c: "c", cpp: "cpp", h: "c", hpp: "cpp",
  java: "java", kotlin: "kotlin", swift: "swift",
  ruby: "ruby", php: "php", r: "r",
  lua: "lua", dart: "dart",
  svelte: "svelte", vue: "vue", astro: "astro",
  txt: "text", gitignore: "text", env: "text",
};

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
  const lang = EXT_TO_LANG[ext] || ext;
  const codeFence = "```" + lang + "\n" + (content ?? "") + "\n```";

  return (
    <div className="h-full flex flex-col">
      <div className="text-xs text-[var(--text-muted)] px-4 py-1.5 border-b border-[var(--border-base)] shrink-0">
        {filePath}
        <span className="ml-2">— {totalLines} lines</span>
      </div>
      <div className="flex-1 overflow-auto p-0">
        <div className="markdown-body !p-0 !m-0">
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            rehypePlugins={[[rehypePrettyCode, { theme: "github-dark-dimmed" }]]}
          >
            {codeFence}
          </ReactMarkdown>
        </div>
      </div>
    </div>
  );
}
