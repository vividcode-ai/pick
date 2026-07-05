import { useRef, useEffect, useLayoutEffect, useState, useCallback } from "react";
import { Terminal, Code2, FolderTree } from "lucide-react";

interface AddMenuProps {
  onNewTerminal: () => void;
  onCodeReview: () => void;
  onFileBrowser: () => void;
  onClose: () => void;
}

const items = [
  { id: "terminal", label: "New Terminal", icon: Terminal, description: "Open a new shell terminal" },
  { id: "codereview", label: "Code Review", icon: Code2, description: "AI-powered code change review" },
  { id: "filebrowser", label: "File Browser", icon: FolderTree, description: "Browse project files" },
] as const;

export function AddMenu({ onNewTerminal, onCodeReview, onFileBrowser, onClose }: AddMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const highlightRef = useRef(0);
  const [alignRight, setAlignRight] = useState(false);

  useLayoutEffect(() => {
    if (menuRef.current) {
      const rect = menuRef.current.getBoundingClientRect();
      if (rect.right > window.innerWidth - 10) {
        setAlignRight(true);
      }
    }
  }, []);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    setTimeout(() => document.addEventListener("mousedown", handler), 0);
    return () => document.removeEventListener("mousedown", handler);
  }, [onClose]);

  const handleSelect = useCallback((id: string) => {
    switch (id) {
      case "terminal": onNewTerminal(); break;
      case "codereview": onCodeReview(); break;
      case "filebrowser": onFileBrowser(); break;
    }
    onClose();
  }, [onNewTerminal, onCodeReview, onFileBrowser, onClose]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      highlightRef.current = Math.min(highlightRef.current + 1, items.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      highlightRef.current = Math.max(highlightRef.current - 1, 0);
    } else if (e.key === "Enter") {
      e.preventDefault();
      handleSelect(items[highlightRef.current].id);
    } else if (e.key === "Escape") {
      onClose();
    }
  }, [handleSelect, onClose]);

  return (
    <>
      <div className="fixed inset-0 z-[2199]" onClick={onClose} />
      <div
        ref={menuRef}
        className={`absolute top-full mt-1 selector-popover z-[2200] min-w-[200px] max-w-[240px] ${alignRight ? 'right-0' : 'left-0'}`}
        onKeyDown={handleKeyDown}
      >
        {items.map((item) => {
          const idx = items.indexOf(item);
          const Icon = item.icon;
          return (
            <div
              key={item.id}
              className="selector-option"
              data-highlighted={idx === highlightRef.current}
              onClick={() => handleSelect(item.id)}
              onMouseEnter={() => { highlightRef.current = idx; }}
            >
              <Icon className="w-4 h-4 shrink-0 mt-0.5 text-[var(--text-muted)]" />
              <div className="selector-option-content">
                <span className="selector-option-label">{item.label}</span>
                <span className="selector-option-description">{item.description}</span>
              </div>
            </div>
          );
        })}
      </div>
    </>
  );
}
