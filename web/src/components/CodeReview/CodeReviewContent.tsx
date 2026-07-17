import { useState, useCallback, useEffect, useRef, useMemo } from "react";
import { Loader2, Play, CheckCircle, AlertCircle, FileText, GitBranch } from "lucide-react";
import { DiffViewer } from "../Chat/DiffViewer";
import { ReviewAccordion, type AccordionItem } from "./ReviewAccordion";
import { ReviewHeader } from "./ReviewHeader";
import { createScrollSaver, getReviewScroll, createVisibilityTracker } from "./ReviewVirtualScroll";
import { subscribeToComments, getCommentsByFile, addComment, loadCommentsFromServer } from "../../stores/comments";
import type { GitDiffEntry, GitDiffsResponse, GitInfo } from "../../types/events";

type DiffSource = "git" | "branch";
type DiffStyle = "unified" | "split";
type ReviewState = "idle" | "streaming" | "done" | "error";

interface CodeReviewContentProps {
  baseUrl: string;
  sessionId: string | null;
  onAsk?: ((prompt: string) => void) | null;
  provider?: string;
  modelId?: string;
  currentCwd?: string;
}

export function CodeReviewContent({ baseUrl, sessionId, onAsk, provider, modelId, currentCwd }: CodeReviewContentProps) {
  // ── URL builder: use session endpoint when available, standalone otherwise ──
  const gitApi = useCallback((path: string) => {
    const base = sessionId ? `${baseUrl}/sessions/${sessionId}` : baseUrl;
    return `${base}${path}`;
  }, [baseUrl, sessionId]);

  // ── Git diffs state ──
  const [gitInfo, setGitInfo] = useState<GitInfo | null>(null);
  const [gitDiffs, setGitDiffs] = useState<GitDiffEntry[]>([]);
  const [loadingDiffs, setLoadingDiffs] = useState(false);
  const [diffError, setDiffError] = useState<string | null>(null);

  // ── Progressive patch loading ──
  const [patches, setPatches] = useState<Record<string, string>>({});
  const [loadingPatches, setLoadingPatches] = useState<Record<string, boolean>>({});

  // ── Settings ──
  const [diffSource, setDiffSource] = useState<DiffSource>("git");
  const [diffStyle, setDiffStyle] = useState<DiffStyle>("unified");
  const [branchName, setBranchName] = useState("main");
  const [branchSuggestions, setBranchSuggestions] = useState<string[]>([]);
  const [expandedFiles, setExpandedFiles] = useState<string[]>([]);
  const [commentPanelOpen, setCommentPanelOpen] = useState(false);

  // ── AI Review state ──
  const [reviewState, setReviewState] = useState<ReviewState>("idle");
  const [reviewText, setReviewText] = useState("");
  const [reviewError, setReviewError] = useState<string | null>(null);
  const eventSourceRef = useRef<EventSource | null>(null);

  // ── Feedback ──
  const [feedbackMsg, setFeedbackMsg] = useState<string | null>(null);
  const feedbackTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // ── Comments ──
  const [allComments, setAllComments] = useState<number>(0);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const scrollSaverRef = useRef<((top: number) => void) | null>(null);
  const visibilityRef = useRef<ReturnType<typeof createVisibilityTracker> | null>(null);
  const visibilityReadyRef = useRef(false);

  // ── Build params for git API calls ──
  const diffParams = useCallback(() => {
    const params = new URLSearchParams();
    if (diffSource === "branch" && branchName) {
      params.set("base", branchName);
    }
    return params;
  }, [diffSource, branchName]);

  // ── Fetch git info ──
  useEffect(() => {
    fetch(`${gitApi("/git-info")}`)
      .then(async (res) => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return res.json();
      })
      .then((data: GitInfo) => setGitInfo(data))
      .catch(() => {});
  }, [gitApi, currentCwd]);

  // ── Fetch branch suggestions ──
  useEffect(() => {
    fetch(`${gitApi("/branches")}`)
      .then((r) => r.json())
      .then((data: string[]) => setBranchSuggestions(data))
      .catch(() => {});
  }, [gitApi, currentCwd]);

  // ── Fetch git diffs (meta only = instant) ──
  useEffect(() => {
    setLoadingDiffs(true);
    setDiffError(null);
    setExpandedFiles([]);
    setPatches({});

    const params = diffParams();
    params.set("meta_only", "true");

    fetch(`${gitApi("/git-diffs")}?${params}`)
      .then(async (res) => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return res.json();
      })
      .then((data: GitDiffsResponse) => {
        setGitDiffs(data.files.filter((f) => !f.binary));
        setLoadingDiffs(false);
      })
      .catch((e) => {
        setDiffError(e.message || "Failed to load diffs");
        setLoadingDiffs(false);
      });
  }, [gitApi, diffParams, currentCwd]);

  // ── Load patch on demand when a file is expanded ──
  const loadPatch = useCallback(async (filePath: string) => {
    if (patches[filePath] !== undefined || loadingPatches[filePath]) return;

    setLoadingPatches((prev) => ({ ...prev, [filePath]: true }));

    try {
      const params = new URLSearchParams({ file: filePath });
      const res = await fetch(`${gitApi("/git-diff")}?${params}`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const entry: GitDiffEntry = await res.json();
      setPatches((prev) => ({ ...prev, [filePath]: entry.patch }));
    } catch (e) {
      console.error("Failed to load patch for", filePath, e);
    }
    setLoadingPatches((prev) => ({ ...prev, [filePath]: false }));
  }, [gitApi, patches, loadingPatches]);

  // Auto-load patch when accordion item opens
  const handleOpenChange = useCallback((next: string[]) => {
    // Newly opened files
    const newlyOpened = next.filter((f) => !expandedFiles.includes(f));
    for (const file of newlyOpened) {
      loadPatch(file);
    }
    setExpandedFiles(next);
  }, [expandedFiles, loadPatch]);

  // ── Load comments from server ──
  useEffect(() => {
    if (!sessionId || !baseUrl) return;
    loadCommentsFromServer(baseUrl, sessionId).catch(() => {});
    const unsub = subscribeToComments(() => {
      let count = 0;
      for (const f of gitDiffs) {
        count += getCommentsByFile(f.path).length;
      }
      setAllComments(count);
    });
    return () => { unsub?.(); };
  }, [baseUrl, sessionId, gitDiffs]);

  // ── Scroll persistence ──
  useEffect(() => {
    if (!sessionId) return;
    scrollSaverRef.current = createScrollSaver(sessionId);
    const saved = getReviewScroll(sessionId);
    if (saved && scrollContainerRef.current) {
      requestAnimationFrame(() => {
        if (scrollContainerRef.current) {
          scrollContainerRef.current.scrollTop = saved;
        }
      });
    }
  }, [sessionId]);

  // ── Visibility tracker for virtual rendering ──
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || visibilityReadyRef.current) return;
    visibilityRef.current = createVisibilityTracker(container, 300);
    visibilityReadyRef.current = true;
    return () => {
      visibilityRef.current?.destroy();
      visibilityReadyRef.current = false;
    };
  }, [gitDiffs]);

  // ── Auto-clear feedback ──
  const showFeedback = useCallback((msg: string) => {
    setFeedbackMsg(msg);
    if (feedbackTimer.current) clearTimeout(feedbackTimer.current);
    feedbackTimer.current = setTimeout(() => setFeedbackMsg(null), 4000);
  }, []);

  // ── Cleanup SSE on unmount ──
  useEffect(() => {
    return () => {
      eventSourceRef.current?.close();
    };
  }, []);

  // ── Handle AI Review ──
  const handleStartReview = useCallback(async () => {
    setReviewState("streaming");
    setReviewText("");
    setReviewError(null);

    // Close any existing EventSource
    eventSourceRef.current?.close();

    try {
      // ── Resolve session: use existing or auto-create ──
      let sid = sessionId;
      if (!sid) {
        const body: Record<string, string> = {};
        if (provider) body.provider = provider;
        if (modelId) body.model_id = modelId;
        const createRes = await fetch(`${baseUrl}/sessions`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(body),
        });
        if (!createRes.ok) throw new Error("Failed to create session");
        const { session_id } = await createRes.json();
        sid = session_id;
      }

      // ── Establish SSE connection (must be open before /review) ──
      const es = new EventSource(`${baseUrl}/events/${sid}`);
      eventSourceRef.current = es;

      await Promise.race([
        new Promise<void>((resolve, reject) => {
          es.onopen = () => resolve();
          es.onerror = () => reject(new Error("SSE connection failed"));
        }),
        new Promise<void>((resolve) => setTimeout(() => resolve(), 5000)),
      ]);

      // ── Start review ──
      const reviewRes = await fetch(`${baseUrl}/review/${sid}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });
      if (!reviewRes.ok) throw new Error(`HTTP ${reviewRes.status}`);

      // ── Listen for streaming results ──
      es.addEventListener("message_update", (e) => {
        try {
          const payload = JSON.parse(e.data);
          let displayText = "";
          if (payload.thinking) {
            displayText += payload.thinking;
          }
          if (payload.text) {
            if (displayText) displayText += "\n\n";
            displayText += payload.text;
          }
          if (displayText) {
            setReviewText(displayText);
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
  }, [baseUrl, sessionId, provider, modelId]);

  // ── Apply AI review as inline comments ──
  const handleApplyAsComments = useCallback(() => {
    let count = 0;
    let totalFiles = 0;

    // Strategy 1: Precise FILE:LINE - description format
    const exactPattern = /(\S+?):(\d+)\s*[-–—]\s*(.+?)(?=\n\S+?:\d+\s*[-–—]|\n\n|$)/gs;
    let match;
    while ((match = exactPattern.exec(reviewText)) !== null) {
      const [, fPath, lineStr, comment] = match;
      const line = parseInt(lineStr, 10);
      if (fPath && !isNaN(line) && comment) {
        addComment({ file: fPath, line, comment: comment.trim(), resolved: false });
        count++;
      }
    }

    // Strategy 2: Fallback — line-beginning `file:line` or file:line - desc
    if (count === 0) {
      const fallbackPattern = /(?:^|\n)\s*`?([\w./\\\-_]+)`?\s*:\s*(\d+)\s*[-–—]\s*(.+?)(?=\n|$)/gm;
      while ((match = fallbackPattern.exec(reviewText)) !== null) {
        const [, fPath, lineStr, comment] = match;
        const line = parseInt(lineStr, 10);
        if (fPath && !isNaN(line) && comment) {
          addComment({ file: fPath, line, comment: comment.trim(), resolved: false });
          count++;
        }
      }
    }

    if (count > 0) {
      const filesWithComments = gitDiffs
        .filter((f) => getCommentsByFile(f.path).length > 0)
        .map((f) => f.path);
      totalFiles = filesWithComments.length;
      setExpandedFiles((prev) => [...new Set([...prev, ...filesWithComments])]);
      showFeedback(`Applied ${count} inline comment(s) across ${totalFiles} file(s)`);
    } else {
      showFeedback("No FILE:LINE entries found — AI may have output reasoning instead of structured findings.");
    }
  }, [reviewText, gitDiffs, showFeedback]);

  // ── Helper ──
  const allFilePaths = useMemo(() => gitDiffs.map((f) => f.path), [gitDiffs]);

  const handleScroll = useCallback(() => {
    const el = scrollContainerRef.current;
    if (!el || !sessionId || !scrollSaverRef.current) return;
    scrollSaverRef.current(el.scrollTop);
  }, [sessionId]);

  // ── Build accordion items ──
  const accordionItems: AccordionItem[] = useMemo(() => gitDiffs.map((diff) => ({
    value: diff.path,
    disabled: diff.binary,
    header: (
      <div className="review-file-info" data-file={diff.path}>
        <span className={`review-status-badge ${
          diff.status === "A" ? "review-status-added" :
          diff.status === "D" ? "review-status-deleted" :
          diff.status === "R" ? "review-status-renamed" :
          diff.status === "??" ? "review-status-untracked" :
          "review-status-modified"
        }`}>
          {diff.status}
        </span>
        <FileText className="w-3.5 h-3.5 shrink-0 text-[var(--text-muted)]" />
        <div className="min-w-0 flex-1 flex items-baseline gap-1 overflow-hidden">
          {diff.path.includes("/") && (
            <span className="review-file-directory">{diff.path.substring(0, diff.path.lastIndexOf("/") + 1)}</span>
          )}
          <span className="review-file-filename">{diff.path.split("/").pop() || diff.path}</span>
        </div>
        <span className="review-changes">
          <span className="review-changes-additions">+{diff.additions}</span>
          <span className="review-changes-separator">/</span>
          <span className="review-changes-deletions">-{diff.deletions}</span>
        </span>
        <div className="review-header-actions">
          <button
            className="review-view-file-btn"
            title="View file"
            onClick={(e) => {
              e.stopPropagation();
              window.open(`${baseUrl}/files/content?path=${encodeURIComponent(diff.path)}`, "_blank");
            }}
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
            </svg>
          </button>
          {getCommentsByFile(diff.path).length > 0 && (
            <span className="review-comment-count">{getCommentsByFile(diff.path).length}</span>
          )}
        </div>
        <span className={`review-chevron ${expandedFiles.includes(diff.path) ? "review-chevron-open" : ""}`}>
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
          </svg>
        </span>
      </div>
    ),
    children: (
      <DiffViewer
        diffText={patches[diff.path] ?? ""}
        filePath={diff.path}
        baseUrl={baseUrl}
        onAsk={onAsk ?? null}
        mode={diffStyle}
        visible={true}
      />
    ),
  })), [gitDiffs, baseUrl, onAsk, diffStyle, expandedFiles, patches]);

  // ── Loading state (initial meta load) ──
  if (loadingDiffs) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
          <Loader2 className="w-3.5 h-3.5 animate-spin" />
          Loading diffs...
        </div>
      </div>
    );
  }

  if (diffError) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="flex items-center gap-2 text-xs text-red-400">
          <AlertCircle className="w-3.5 h-3.5" />
          {diffError}
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <ReviewHeader
        diffSource={diffSource}
        onDiffSourceChange={setDiffSource}
        diffStyle={diffStyle}
        onDiffStyleChange={setDiffStyle}
        branchName={branchName}
        onBranchNameChange={setBranchName}
        branchSuggestions={branchSuggestions}
        expandedCount={expandedFiles.length}
        totalFiles={gitDiffs.length}
        onExpandAll={() => {
          setExpandedFiles(allFilePaths);
          allFilePaths.forEach(loadPatch);
        }}
        onCollapseAll={() => setExpandedFiles([])}
        onStartReview={handleStartReview}
        reviewState={reviewState}
        commentCount={allComments}
        onToggleComments={() => setCommentPanelOpen((v) => !v)}
        commentPanelOpen={commentPanelOpen}
      />

      {/* Git Info Summary */}
      {gitInfo && (
        <div className="px-3 py-1.5 border-b border-[var(--border-base)] flex items-center gap-2 text-xs text-[var(--text-muted)] shrink-0">
          <GitBranch className="w-3 h-3" />
          <span className="font-medium text-[var(--text-primary)]">{gitInfo.branch}</span>
          <span>&mdash; {gitDiffs.length} file{gitDiffs.length !== 1 ? "s" : ""} changed</span>
        </div>
      )}

      {/* Scrollable Content */}
      <div
        ref={scrollContainerRef}
        className="flex-1 overflow-auto"
        onScroll={handleScroll}
      >
        {/* Diffs */}
        {accordionItems.length > 0 ? (
          <ReviewAccordion
            items={accordionItems}
            open={expandedFiles}
            onOpenChange={handleOpenChange}
          />
        ) : (
          <div className="px-3 py-4 text-xs text-[var(--text-muted)]">
            No file changes detected
          </div>
        )}

        {/* AI Review Section */}
        <div className="review-ai-section">
          {reviewState === "idle" && gitDiffs.length === 0 && (
            <div className="text-xs text-[var(--text-muted)]">
              Open a file in the workspace to see changes here.
            </div>
          )}

          {reviewState === "streaming" && (
            <div className="flex flex-col gap-2">
              <div className="flex items-center gap-2 text-xs text-[var(--accent-primary)]">
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                Reviewing code changes...
              </div>
              {reviewText && (
                <div className="review-ai-text">
                  {reviewText}
                  <span className="review-ai-streaming-cursor">▊</span>
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
                <>
                  <div className="review-ai-text">{reviewText}</div>
                  <div className="flex items-center gap-2">
                    <button
                      onClick={handleApplyAsComments}
                      className="review-apply-comments-btn"
                    >
                      Apply as inline comments
                    </button>
                    <button
                      onClick={handleStartReview}
                      className="flex items-center gap-2 px-3 py-1.5 rounded text-xs font-medium bg-[var(--accent-primary)] text-white hover:opacity-90 transition-opacity self-start"
                    >
                      <Play className="w-3.5 h-3.5" />
                      Re-review
                    </button>
                  </div>
                </>
              )}
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

          {feedbackMsg && (
            <div className="px-3 py-2 text-xs text-[var(--text-muted)] border-t border-[var(--border-base)]">
              {feedbackMsg}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
