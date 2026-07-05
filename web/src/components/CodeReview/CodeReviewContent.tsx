import { useState, useCallback, useEffect, useRef } from "react";
import { Loader2, Play, FileText, GitBranch, CheckCircle, AlertCircle } from "lucide-react";
import { DiffViewer } from "../Chat/DiffViewer";
import type { GitInfo } from "../../types/events";

interface CodeReviewContentProps {
  baseUrl: string;
  sessionId: string | null;
}

export function CodeReviewContent({ baseUrl, sessionId }: CodeReviewContentProps) {
  const [gitInfo, setGitInfo] = useState<GitInfo | null>(null);
  const [loadingGit, setLoadingGit] = useState(false);
  const [gitError, setGitError] = useState<string | null>(null);
  const [reviewState, setReviewState] = useState<"idle" | "streaming" | "done" | "error">("idle");
  const [reviewText, setReviewText] = useState("");
  const eventSourceRef = useRef<EventSource | null>(null);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [expandedDiffs, setExpandedDiffs] = useState<Record<string, boolean>>({});
  const [fileDiffs, setFileDiffs] = useState<Record<string, string>>({});
  const [loadingDiffs, setLoadingDiffs] = useState<Record<string, boolean>>({});

  useEffect(() => {
    if (!sessionId) return;
    setLoadingGit(true);
    setGitError(null);
    fetch(`${baseUrl}/sessions/${sessionId}/git-info`)
      .then(async (res) => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return res.json();
      })
      .then((data: GitInfo) => {
        setGitInfo(data);
        setLoadingGit(false);
      })
      .catch((e) => {
        setGitError(e.message || "Failed to load git info");
        setLoadingGit(false);
      });
  }, [baseUrl, sessionId]);

  const handleToggleDiff = useCallback(async (filePath: string) => {
    const isCurrentlyExpanded = expandedDiffs[filePath];
    setExpandedDiffs(prev => ({ ...prev, [filePath]: !isCurrentlyExpanded }));
    if (!isCurrentlyExpanded && !fileDiffs[filePath]) {
      setLoadingDiffs(prev => ({ ...prev, [filePath]: true }));
      try {
        const res = await fetch(`${baseUrl}/files/content?path=${encodeURIComponent(filePath)}`);
        if (res.ok) {
          const data = await res.json();
          if (!data.binary) {
            setFileDiffs(prev => ({ ...prev, [filePath]: data.content }));
          } else {
            setFileDiffs(prev => ({ ...prev, [filePath]: "[Binary file]" }));
          }
        }
      } catch (e) {
        console.error("Failed to load file:", e);
      }
      setLoadingDiffs(prev => ({ ...prev, [filePath]: false }));
    }
  }, [baseUrl, expandedDiffs, fileDiffs]);

  useEffect(() => {
    return () => {
      eventSourceRef.current?.close();
    };
  }, []);

  const handleStartReview = useCallback(async () => {
    if (!sessionId) return;
    setReviewState("streaming");
    setReviewText("");
    setReviewError(null);

    try {
      const res = await fetch(`${baseUrl}/sessions/${sessionId}/git-info`);
      let gitContext = "";
      if (res.ok) {
        const info: GitInfo = await res.json();
        gitContext = `\nCurrent branch: ${info.branch}\nChanged files:\n${
          info.changes.map((c) => `  ${c.status} ${c.path}`).join("\n")
        }`;
      }

      const prompt = `Please review the following code changes and provide a thorough code review. ${gitContext}\n\nPlease analyze the changes for:
1. Potential bugs and logic errors
2. Security issues and risks
3. Code style and best practices
4. Performance optimization suggestions
5. Maintainability and readability improvements

Provide specific suggestions with code examples where applicable.`;

      const askRes = await fetch(`${baseUrl}/ask`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ session_id: sessionId, prompt }),
      });
      if (!askRes.ok) {
        throw new Error(`Failed to start review: HTTP ${askRes.status}`);
      }

      const es = new EventSource(`${baseUrl}/events/${sessionId}`);
      eventSourceRef.current = es;

      es.addEventListener("message_update", (e) => {
        try {
          const payload = JSON.parse(e.data);
          if (payload.text) {
            setReviewText((prev) => prev + payload.text);
          }
        } catch {}
      });

      es.addEventListener("agent_end", () => {
        setReviewState("done");
        es.close();
        eventSourceRef.current = null;
      });

      es.addEventListener("error", () => {
        setReviewState("error");
        setReviewError("Connection lost");
        es.close();
        eventSourceRef.current = null;
      });
    } catch (e: any) {
      setReviewState("error");
      setReviewError(e.message || "Failed to start review");
    }
  }, [baseUrl, sessionId]);

  return (
    <div className="h-full overflow-auto flex flex-col">
      {/* Git Info */}
      <div className="px-3 py-2 border-b border-[var(--border-base)] shrink-0">
        {loadingGit ? (
          <div className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
            <Loader2 className="w-3 h-3 animate-spin" />
            Loading git info...
          </div>
        ) : gitError ? (
          <div className="flex items-center gap-2 text-xs text-red-400">
            <AlertCircle className="w-3 h-3" />
            {gitError}
          </div>
        ) : gitInfo ? (
          <div>
            <div className="flex items-center gap-2 text-xs text-[var(--text-primary)] mb-1">
              <GitBranch className="w-3.5 h-3.5" />
              <span className="font-medium">{gitInfo.branch}</span>
              <span className="text-[var(--text-muted)]">
                — {gitInfo.changes.length} file{gitInfo.changes.length !== 1 ? "s" : ""} changed
              </span>
            </div>
            {gitInfo.changes.length > 0 && (
              <div className="mt-1 space-y-0.5">
                {gitInfo.changes.map((change) => {
                  const statusMap: Record<string, string> = {
                    "M": "Modified", "A": "Added", "D": "Deleted", "R": "Renamed", "??": "Untracked",
                  };
                  const isExpanded = expandedDiffs[change.path] ?? false;
                  return (
                    <div key={change.path}>
                      <div
                        className="flex items-center gap-2 px-2 py-0.5 rounded text-xs cursor-pointer hover:bg-[var(--surface-hover)] transition-colors"
                        onClick={() => handleToggleDiff(change.path)}
                      >
                        <span className={`text-[10px] font-mono font-semibold w-6 ${
                          change.status === "A" ? "text-green-400" :
                          change.status === "D" ? "text-red-400" :
                          change.status === "M" ? "text-amber-400" :
                          change.status === "??" ? "text-blue-400" : "text-[var(--text-muted)]"
                        }`}>
                          {statusMap[change.status] || change.status}
                        </span>
                        <FileText className="w-3 h-3 shrink-0 text-[var(--text-muted)]" />
                        <span className="truncate">{change.path}</span>
                        {loadingDiffs[change.path] && (
                          <Loader2 className="w-3 h-3 animate-spin text-[var(--text-muted)] ml-auto" />
                        )}
                      </div>
                      {isExpanded && fileDiffs[change.path] && (
                        <div className="ml-8 mr-2 mb-1 border border-[var(--border-base)] rounded overflow-hidden">
                          <DiffViewer diffText={fileDiffs[change.path]} />
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        ) : null}
      </div>

      {/* Review Actions & Results */}
      <div className="px-3 py-2 flex-1 overflow-auto">
        {reviewState === "idle" && (
          <button
            onClick={handleStartReview}
            disabled={!gitInfo || !sessionId}
            className="flex items-center gap-2 px-3 py-1.5 rounded text-xs font-medium bg-[var(--accent-primary)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >
            <Play className="w-3.5 h-3.5" />
            Start AI Review
          </button>
        )}

        {reviewState === "streaming" && (
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-2 text-xs text-[var(--accent-primary)]">
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
              Reviewing code changes...
            </div>
            {reviewText && (
              <div className="border border-[var(--border-base)] rounded p-3 text-xs leading-relaxed whitespace-pre-wrap font-sans">
                {reviewText}
                <span className="animate-pulse">▊</span>
              </div>
            )}
          </div>
        )}

        {reviewState === "done" && (
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-2 text-xs text-green-400">
              <CheckCircle className="w-3.5 h-3.5" />
              Review complete
            </div>
            {reviewText && (
              <div className="border border-[var(--border-base)] rounded p-3 text-xs leading-relaxed whitespace-pre-wrap font-sans">
                {reviewText}
              </div>
            )}
            <button
              onClick={handleStartReview}
              className="flex items-center gap-2 px-3 py-1.5 rounded text-xs font-medium bg-[var(--accent-primary)] text-white hover:opacity-90 transition-opacity self-start"
            >
              <Play className="w-3.5 h-3.5" />
              Re-review
            </button>
          </div>
        )}

        {reviewState === "error" && (
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-2 text-xs text-red-400">
              <AlertCircle className="w-3.5 h-3.5" />
              {reviewError || "Review failed"}
            </div>
            <button
              onClick={handleStartReview}
              className="flex items-center gap-2 px-3 py-1.5 rounded text-xs font-medium bg-[var(--accent-primary)] text-white hover:opacity-90 transition-opacity self-start"
            >
              <Play className="w-3.5 h-3.5" />
              Retry
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
