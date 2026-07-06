import { useState } from "react";
import { X } from "lucide-react";

interface CommitModalProps {
  open: boolean;
  onClose: () => void;
  onCommit: (message: string) => void;
}

export function CommitModal({ open, onClose, onCommit }: CommitModalProps) {
  const [message, setMessage] = useState("");

  if (!open) return null;

  const handleSubmit = () => {
    const trimmed = message.trim();
    if (!trimmed) return;
    onCommit(trimmed);
    setMessage("");
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div
        className="w-[400px] rounded-xl border border-[var(--border-base)] bg-[var(--surface-secondary)] shadow-lg"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-base)]">
          <h3 className="text-sm font-semibold text-[var(--text-primary)]">
            Commit Changes
          </h3>
          <button
            onClick={onClose}
            className="p-1 rounded-md hover:bg-[var(--surface-hover)] text-[var(--text-muted)] transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
        <div className="p-4">
          <textarea
            autoFocus
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            placeholder="Enter commit message..."
            className="w-full h-24 px-3 py-2 text-sm rounded-lg border border-[var(--border-base)] bg-[var(--surface-base)] text-[var(--text-primary)] placeholder:text-[var(--text-muted)] resize-none outline-none focus:ring-1 focus:ring-[var(--accent-primary)]"
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                handleSubmit();
              }
            }}
          />
          <div className="flex justify-end gap-2 mt-3">
            <button
              onClick={onClose}
              className="px-3 py-1.5 text-xs font-medium rounded-lg border border-[var(--border-base)] text-[var(--text-muted)] hover:bg-[var(--surface-hover)] transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleSubmit}
              disabled={!message.trim()}
              className="px-3 py-1.5 text-xs font-medium rounded-lg bg-[var(--accent-primary)] text-white hover:bg-[var(--accent-hover)] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              Commit
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
