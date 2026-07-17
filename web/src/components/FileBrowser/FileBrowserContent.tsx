import { useState, useCallback, useRef, useEffect } from "react";
import { Search, X, Loader2 } from "lucide-react";
import { FileTree } from "./FileTree";
import { FilePreview } from "./FilePreview";

const MIN_RIGHT_WIDTH = 200;
const MAX_RIGHT_WIDTH = 600;

interface FileBrowserContentProps {
  baseUrl: string;
  onAsk?: ((prompt: string) => void) | null;
  rootCwd?: string;
}

export function FileBrowserContent({ baseUrl, onAsk, rootCwd }: FileBrowserContentProps) {
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<{ path: string; name: string }[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [showTree, setShowTree] = useState(true);
  const [rightWidth, setRightWidth] = useState(280);
  const resizingRef = useRef(false);

  useEffect(() => {
    setSelectedFile(null);
    setSearchResults(null);
  }, [rootCwd]);

  const handleFileSelect = useCallback((path: string) => {
    setSelectedFile(path);
  }, []);

  const handleSearch = useCallback(async () => {
    const q = searchQuery.trim();
    if (!q) {
      setSearchResults(null);
      return;
    }
    setSearching(true);
    try {
      const res = await fetch(`${baseUrl}/find/files?pattern=${encodeURIComponent(q)}&limit=200`);
      if (!res.ok) return;
      const data = await res.json();
      const results = (data.matches || data.results || data.files || [])
        .map((m: any) => {
          const path = m.path || m.file || m.name || "";
          const parts = path.replace(/\\/g, "/").split("/");
          return { path, name: parts[parts.length - 1] || path };
        });
      setSearchResults(results);
    } catch (e) {
      console.error("Search failed:", e);
    }
    setSearching(false);
  }, [baseUrl, searchQuery]);

  const handleClearSearch = useCallback(() => {
    setSearchQuery("");
    setSearchResults(null);
  }, []);

  const handleSearchKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Enter") handleSearch();
    if (e.key === "Escape") handleClearSearch();
  }, [handleSearch, handleClearSearch]);

  const handleResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    resizingRef.current = true;
    const startX = e.clientX;
    const startWidth = rightWidth;

    const handleMouseMove = (ev: MouseEvent) => {
      if (!resizingRef.current) return;
      const delta = startX - ev.clientX;
      const newWidth = Math.max(MIN_RIGHT_WIDTH, Math.min(MAX_RIGHT_WIDTH, startWidth + delta));
      setRightWidth(newWidth);
    };

    const handleMouseUp = () => {
      resizingRef.current = false;
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };

    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
  }, [rightWidth]);

  return (
    <div className="flex h-full">
      {/* Left: File Preview */}
      <div className="flex-1 min-w-0">
        <FilePreview baseUrl={baseUrl} filePath={selectedFile} onAsk={onAsk} />
      </div>

      {/* Resize handle */}
      <div
        onMouseDown={handleResizeStart}
        className="w-[5px] shrink-0 cursor-col-resize bg-transparent hover:bg-[var(--accent-primary)] active:bg-[var(--accent-primary)] transition-colors z-10"
      />

      {/* Right: Search + Tree */}
      <div
        className="shrink-0 flex flex-col bg-[var(--surface-base)]"
        style={{ width: rightWidth }}
      >
        <div className="px-2 py-2 border-b border-[var(--border-base)] shrink-0">
          <div className="relative">
            <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-[var(--text-muted)]" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={handleSearchKeyDown}
              placeholder="Search files..."
              className="selector-search-input pl-7 pr-7 text-xs"
            />
            {searchQuery && (
              <button
                onClick={handleClearSearch}
                className="absolute right-1 top-1/2 -translate-y-1/2 p-0.5 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)]"
              >
                <X className="w-3 h-3" />
              </button>
            )}
          </div>
          <div className="flex gap-1 mt-1">
            <button
              onClick={() => { setShowTree(true); setSearchResults(null); }}
              className={`px-2 py-0.5 text-[10px] rounded transition-colors ${
                showTree && !searchResults
                  ? "bg-[var(--accent-primary)]/10 text-[var(--accent-primary)]"
                  : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
              }`}
            >
              Tree
            </button>
            <button
              onClick={handleSearch}
              disabled={!searchQuery.trim()}
              className={`px-2 py-0.5 text-[10px] rounded transition-colors ${
                searchResults
                  ? "bg-[var(--accent-primary)]/10 text-[var(--accent-primary)]"
                  : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)] disabled:opacity-40"
              }`}
            >
              Search
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-auto">
          {searching ? (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="w-4 h-4 animate-spin text-[var(--text-muted)]" />
            </div>
          ) : searchResults ? (
            <div>
              <div className="text-[10px] text-[var(--text-muted)] px-3 py-1 border-b border-[var(--border-base)]">
                {searchResults.length} results
              </div>
              {searchResults.map((r) => (
                <div
                  key={r.path}
                  className={`flex items-center gap-2 px-3 py-1 text-xs cursor-pointer hover:bg-[var(--surface-hover)] transition-colors ${
                    selectedFile === r.path ? "bg-[var(--accent-primary)]/10 text-[var(--accent-primary)]" : ""
                  }`}
                  onClick={() => setSelectedFile(r.path)}
                >
                  <span className="text-xs">📄</span>
                  <span className="whitespace-nowrap">{r.path}</span>
                </div>
              ))}
            </div>
          ) : showTree ? (
            <div className="py-1">
              <FileTree
                baseUrl={baseUrl}
                rootPath={rootCwd || "."}
                onFileSelect={handleFileSelect}
                selectedFile={selectedFile}
                key={rootCwd || "."}
              />
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}
