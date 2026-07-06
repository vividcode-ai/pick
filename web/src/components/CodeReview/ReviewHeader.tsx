import { useCallback, useRef, useState, type ReactNode } from "react";
import { GitBranch, MessageSquare, Play, Loader2 } from "lucide-react";

type DiffSource = "git" | "branch";
type DiffStyle = "unified" | "split";
type ReviewState = "idle" | "streaming" | "done" | "error";

interface ReviewHeaderProps {
  diffSource: DiffSource;
  onDiffSourceChange: (source: DiffSource) => void;
  diffStyle: DiffStyle;
  onDiffStyleChange: (style: DiffStyle) => void;
  branchName: string;
  onBranchNameChange: (name: string) => void;
  branchSuggestions: string[];
  expandedCount: number;
  totalFiles: number;
  onExpandAll: () => void;
  onCollapseAll: () => void;
  onStartReview: () => void;
  reviewState: ReviewState;
  commentCount: number;
  onToggleComments: () => void;
  commentPanelOpen: boolean;
}

export function ReviewHeader({
  diffSource,
  onDiffSourceChange,
  diffStyle,
  onDiffStyleChange,
  branchName,
  onBranchNameChange,
  branchSuggestions,
  expandedCount,
  totalFiles,
  onExpandAll,
  onCollapseAll,
  onStartReview,
  reviewState,
  commentCount,
  onToggleComments,
  commentPanelOpen,
}: ReviewHeaderProps) {
  const [sourceOpen, setSourceOpen] = useState(false);
  const [branchInputOpen, setBranchInputOpen] = useState(false);
  const [branchInput, setBranchInput] = useState(branchName);
  const [showBranchSuggestions, setShowBranchSuggestions] = useState(false);
  const branchRef = useRef<HTMLDivElement>(null);

  const handleSourceSelect = useCallback((source: DiffSource) => {
    onDiffSourceChange(source);
    setSourceOpen(false);
    if (source === "branch") {
      setBranchInputOpen(true);
    } else {
      setBranchInputOpen(false);
    }
  }, [onDiffSourceChange]);

  const handleBranchSubmit = useCallback(() => {
    if (branchInput.trim()) {
      onBranchNameChange(branchInput.trim());
      setBranchInputOpen(false);
    }
  }, [branchInput, onBranchNameChange]);

  const handleBranchInputKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      handleBranchSubmit();
    } else if (e.key === "Escape") {
      setBranchInputOpen(false);
      setBranchInput(branchName);
    }
  }, [handleBranchSubmit, branchName]);

  const allExpanded = expandedCount === totalFiles && totalFiles > 0;

  return (
    <div className="review-header px-3 py-1.5 border-b border-[var(--border-base)] bg-[var(--surface-base)] flex items-center gap-2 text-xs shrink-0 flex-wrap">
      {/* Title */}
      <span className="font-medium text-[var(--text-primary)] mr-1">Code Review</span>

      {/* Diff Source Selector */}
      <div className="relative">
        <button
          onClick={() => setSourceOpen((v) => !v)}
          className="flex items-center gap-1 px-2 py-1 rounded border border-[var(--border-base)] hover:bg-[var(--surface-hover)] text-[var(--text-muted)]"
        >
          <GitBranch className="w-3 h-3" />
          <span>{diffSource === "git" ? "Git Changes" : "Branch"}</span>
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
          </svg>
        </button>
        {sourceOpen && (
          <>
            <div className="fixed inset-0 z-10" onClick={() => setSourceOpen(false)} />
            <div className="absolute top-full left-0 mt-1 z-20 bg-[var(--surface-base)] border border-[var(--border-base)] rounded shadow-lg min-w-[140px]">
              <button
                className={`w-full text-left px-3 py-1.5 hover:bg-[var(--surface-hover)] ${diffSource === "git" ? "text-[var(--accent-primary)]" : "text-[var(--text-primary)]"}`}
                onClick={() => handleSourceSelect("git")}
              >
                Git Changes
              </button>
              <button
                className={`w-full text-left px-3 py-1.5 hover:bg-[var(--surface-hover)] ${diffSource === "branch" ? "text-[var(--accent-primary)]" : "text-[var(--text-primary)]"}`}
                onClick={() => handleSourceSelect("branch")}
              >
                Branch Diff
              </button>
            </div>
          </>
        )}
      </div>

      {/* Branch Name Input (shown in branch mode) */}
      {branchInputOpen && (
        <div ref={branchRef} className="relative">
          <input
            autoFocus
            value={branchInput}
            onChange={(e) => {
              setBranchInput(e.target.value);
              setShowBranchSuggestions(true);
            }}
            onKeyDown={handleBranchInputKeyDown}
            onFocus={() => setShowBranchSuggestions(true)}
            onBlur={() => setTimeout(() => setShowBranchSuggestions(false), 200)}
            placeholder="branch name..."
            className="w-28 px-2 py-1 text-xs rounded border border-[var(--border-base)] bg-[var(--surface-base)] text-[var(--text-primary)] outline-none focus:border-[var(--accent-primary)]"
          />
          {showBranchSuggestions && branchInput && branchSuggestions.length > 0 && (
            <div className="absolute top-full left-0 mt-1 z-20 bg-[var(--surface-base)] border border-[var(--border-base)] rounded shadow-lg min-w-[120px] max-h-32 overflow-auto">
              {branchSuggestions
                .filter((b) => b.toLowerCase().includes(branchInput.toLowerCase()))
                .slice(0, 10)
                .map((b) => (
                  <button
                    key={b}
                    className="block w-full text-left px-3 py-1 hover:bg-[var(--surface-hover)] text-[var(--text-primary)]"
                    onMouseDown={() => {
                      setBranchInput(b);
                      onBranchNameChange(b);
                      setBranchInputOpen(false);
                    }}
                  >
                    {b}
                  </button>
                ))}
            </div>
          )}
        </div>
      )}

      <span className="text-[var(--text-muted)]">|</span>

      {/* Diff Style Toggle */}
      <div className="flex items-center border border-[var(--border-base)] rounded overflow-hidden">
        <button
          onClick={() => onDiffStyleChange("unified")}
          className={`flex items-center gap-1 px-2 py-1 text-xs ${
            diffStyle === "unified"
              ? "bg-[var(--accent-primary)] text-white"
              : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
          }`}
        >
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 10h16M4 14h16M4 18h16" />
          </svg>
          Unified
        </button>
        <button
          onClick={() => onDiffStyleChange("split")}
          className={`flex items-center gap-1 px-2 py-1 text-xs ${
            diffStyle === "split"
              ? "bg-[var(--accent-primary)] text-white"
              : "text-[var(--text-muted)] hover:bg-[var(--surface-hover)]"
          }`}
        >
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 3v18M4 6h7M4 10h7M4 14h7M4 18h7M13 6h7M13 10h7M13 14h7M13 18h7" />
          </svg>
          Split
        </button>
      </div>

      <span className="text-[var(--text-muted)]">|</span>

      {/* Expand/Collapse All */}
      {totalFiles > 0 && (
        <button
          onClick={allExpanded ? onCollapseAll : onExpandAll}
          className="flex items-center gap-1 px-2 py-1 rounded hover:bg-[var(--surface-hover)] text-[var(--text-muted)]"
        >
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            {allExpanded ? (
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
            ) : (
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            )}
          </svg>
          {allExpanded ? "Collapse All" : "Expand All"}
        </button>
      )}

      <div className="flex-1" />

      {/* Comment Count Badge */}
      <button
        onClick={onToggleComments}
        className={`flex items-center gap-1 px-2 py-1 rounded hover:bg-[var(--surface-hover)] ${
          commentPanelOpen ? "text-[var(--accent-primary)]" : "text-[var(--text-muted)]"
        }`}
      >
        <MessageSquare className="w-3 h-3" />
        <span>{commentCount}</span>
      </button>

      {/* Start AI Review Button */}
      <button
        onClick={onStartReview}
        disabled={reviewState === "streaming" || totalFiles === 0}
        className="flex items-center gap-1 px-3 py-1 rounded text-xs font-medium bg-[var(--accent-primary)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
      >
        {reviewState === "streaming" ? (
          <Loader2 className="w-3 h-3 animate-spin" />
        ) : (
          <Play className="w-3 h-3" />
        )}
        {reviewState === "streaming" ? "Reviewing..." : reviewState === "done" ? "Re-review" : "Start AI Review"}
      </button>
    </div>
  );
}
