import { useState, useRef, useEffect, useCallback } from "react";
import { ChevronDown, Check } from "lucide-react";
import type { ProviderInfo } from "../../types/events";

interface ThinkingSelectorProps {
  providers: ProviderInfo[];
  selectedModel: string;
  thinkingLevel: string;
  onThinkingLevelChange: (l: string) => void;
  disabled?: boolean;
}

const THINKING_LEVELS = [
  { value: "off", label: "Off" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
];

export function ThinkingSelector({
  providers,
  selectedModel,
  thinkingLevel,
  onThinkingLevelChange,
  disabled,
}: ThinkingSelectorProps) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const [highlightIdx, setHighlightIdx] = useState(0);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const modelSupportsReasoning = providers
    .flatMap((p) => p.models)
    .some((m) => m.id === selectedModel && m.reasoning);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!open) return;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setHighlightIdx((i) => Math.min(i + 1, THINKING_LEVELS.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setHighlightIdx((i) => Math.max(i - 1, 0));
      } else if (e.key === "Enter" && THINKING_LEVELS[highlightIdx]) {
        e.preventDefault();
        onThinkingLevelChange(THINKING_LEVELS[highlightIdx].value);
        setOpen(false);
      } else if (e.key === "Escape") {
        setOpen(false);
      }
    },
    [open, highlightIdx, onThinkingLevelChange]
  );

  if (!modelSupportsReasoning) return null;

  const selectedLabel = THINKING_LEVELS.find((l) => l.value === thinkingLevel)?.label || "Off";

  return (
    <div className="relative w-fit" ref={containerRef} onKeyDown={handleKeyDown}>
      <button
        onClick={() => { setOpen((v) => !v); setHighlightIdx(0); }}
        disabled={disabled}
        className="selector-trigger w-fit"
      >
        <span className="selector-trigger-primary">{selectedLabel}</span>
        <span className="selector-trigger-icon">
          <ChevronDown className="w-3 h-3" />
        </span>
      </button>

      {open && (
        <>
          <div className="fixed inset-0 z-[2199]" onClick={() => setOpen(false)} />
          <div className="absolute bottom-full left-0 mb-2 selector-popover z-[2200]">
            <div className="selector-listbox">
              {THINKING_LEVELS.map((level, idx) => {
                const selected = level.value === thinkingLevel;
                return (
                  <div
                    key={level.value}
                    className="selector-option"
                    data-highlighted={idx === highlightIdx}
                    data-selected={selected}
                    onClick={() => { onThinkingLevelChange(level.value); setOpen(false); }}
                    onMouseEnter={() => setHighlightIdx(idx)}
                  >
                    <div className="selector-option-content">
                      <span className="selector-option-label">{level.label}</span>
                    </div>
                    {selected && (
                      <span className="selector-option-indicator">
                        <Check className="w-3.5 h-3.5" />
                      </span>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
