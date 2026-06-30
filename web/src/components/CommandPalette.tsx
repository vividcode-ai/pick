import { useState, useMemo, useEffect, useRef } from "react";
import { Search } from "lucide-react";
import type { Command } from "../stores/commands";

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  commands: Command[];
  onExecute: (command: Command) => void;
}

export function CommandPalette({ open, onClose, commands, onExecute }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter((cmd) => {
      const label = cmd.label.toLowerCase();
      const desc = cmd.description?.toLowerCase() ?? "";
      const keywords = cmd.keywords?.some((k) => k.toLowerCase().includes(q)) ?? false;
      const category = cmd.category?.toLowerCase() ?? "";
      return label.includes(q) || desc.includes(q) || keywords || category.includes(q);
    });
  }, [commands, query]);

  const grouped = useMemo(() => {
    const groups = new Map<string, Command[]>();
    for (const cmd of filtered) {
      const cat = cmd.category ?? "Other";
      if (!groups.has(cat)) groups.set(cat, []);
      groups.get(cat)!.push(cmd);
    }
    return groups;
  }, [filtered]);

  const flatList = useMemo(() => filtered, [filtered]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, flatList.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter" && flatList[selectedIndex]) {
      e.preventDefault();
      onExecute(flatList[selectedIndex]);
      onClose();
    } else if (e.key === "Escape") {
      onClose();
    }
  };

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[100] flex items-start justify-center pt-[15vh]"
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg bg-neutral-900 border border-neutral-700 rounded-xl shadow-2xl overflow-hidden"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <div className="flex items-center gap-2 px-4 py-3 border-b border-neutral-700">
          <Search className="w-4 h-4 text-neutral-500" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => { setQuery(e.target.value); setSelectedIndex(0); }}
            placeholder="Type a command..."
            className="flex-1 bg-transparent text-sm text-neutral-100 outline-none placeholder-neutral-500"
          />
          <span className="text-[10px] text-neutral-500 px-1.5 py-0.5 rounded bg-neutral-800">ESC</span>
        </div>

        <div className="max-h-80 overflow-y-auto py-2">
          {flatList.length === 0 ? (
            <div className="text-sm text-neutral-500 text-center py-8">No results found</div>
          ) : (
            Array.from(grouped.entries()).map(([category, cmds]) => (
              <div key={category}>
                <div className="px-4 py-1 text-[10px] font-semibold uppercase text-neutral-500 tracking-wider">
                  {category}
                </div>
                {cmds.map((cmd, idx) => {
                  const globalIdx = flatList.indexOf(cmd);
                  return (
                    <button
                      key={cmd.id}
                      className={`w-full flex items-center justify-between px-4 py-2 text-sm text-left transition-colors ${
                        globalIdx === selectedIndex
                          ? "bg-blue-600/20 text-blue-300"
                          : "text-neutral-300 hover:bg-neutral-800"
                      }`}
                      onClick={() => { onExecute(cmd); onClose(); }}
                      onMouseEnter={() => setSelectedIndex(globalIdx)}
                    >
                      <span>{cmd.label}</span>
                      {cmd.shortcut && (
                        <span className="text-[10px] text-neutral-500 px-1.5 py-0.5 rounded bg-neutral-800">
                          {cmd.shortcut}
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
