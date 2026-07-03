import { useRef, useState, useEffect, useMemo, type KeyboardEvent } from "react";
import { HelpCircle } from "lucide-react";
import type { QuestionPayload } from "../../types/events";

interface QuestionDialogProps {
  payload: QuestionPayload;
  onSubmit: (answers: string[][]) => void;
  onCancel: () => void;
}

interface FocusItem {
  type: "option";
  promptIdx: number;
  optionIdx: number;
  label: string;
}

export function QuestionDialog({ payload, onSubmit, onCancel }: QuestionDialogProps) {
  const [selections, setSelections] = useState<string[][]>(
    payload.prompts.map(() => [])
  );
  const [focusIdx, setFocusIdx] = useState(0);
  const btnRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const allItems = useMemo(() => {
    const items: (FocusItem | { type: "cancel" } | { type: "submit" })[] = [];
    for (let pi = 0; pi < payload.prompts.length; pi++) {
      for (let oi = 0; oi < payload.prompts[pi].options.length; oi++) {
        items.push({ type: "option", promptIdx: pi, optionIdx: oi, label: payload.prompts[pi].options[oi].label });
      }
    }
    items.push({ type: "cancel" });
    items.push({ type: "submit" });
    return items;
  }, [payload.prompts]);

  useEffect(() => {
    btnRefs.current[focusIdx]?.focus();
  }, [focusIdx]);

  useEffect(() => {
    btnRefs.current[0]?.focus();
  }, []);

  const handleToggle = (promptIdx: number, label: string) => {
    setSelections((prev) => {
      const next = prev.map((s) => [...s]);
      const prompt = payload.prompts[promptIdx];
      if (prompt.multiple) {
        const idx = next[promptIdx].indexOf(label);
        if (idx >= 0) {
          next[promptIdx].splice(idx, 1);
        } else {
          next[promptIdx].push(label);
        }
      } else {
        next[promptIdx] = [label];
      }
      return next;
    });
  };

  const allAnswered = selections.every((s, i) =>
    payload.prompts[i].multiple ? s.length > 0 : s.length === 1
  );

  const handleKeyDown = (e: KeyboardEvent) => {
    switch (e.key) {
      case "ArrowUp":
        e.preventDefault();
        setFocusIdx((prev) => Math.max(0, prev - 1));
        break;
      case "ArrowDown":
        e.preventDefault();
        setFocusIdx((prev) => Math.min(allItems.length - 1, prev + 1));
        break;
      case "Tab":
        e.preventDefault();
        setFocusIdx((prev) => {
          if (e.shiftKey) return Math.max(0, prev - 1);
          return Math.min(allItems.length - 1, prev + 1);
        });
        break;
      case "Enter":
        e.preventDefault();
        {
          const item = allItems[focusIdx];
          if (item.type === "option") {
            const prompt = payload.prompts[item.promptIdx];
            const nextSelections = selections.map((s, i) => {
              if (i !== item.promptIdx) return [...s];
              if (prompt.multiple) {
                const idx = s.indexOf(item.label);
                if (idx >= 0) return s.filter((_, j) => j !== idx);
                return [...s, item.label];
              }
              return [item.label];
            });
            setSelections(nextSelections);
            if (nextSelections.every((s, i) =>
              payload.prompts[i].multiple ? s.length > 0 : s.length === 1
            )) {
              onSubmit(nextSelections);
            }
          } else if (item.type === "cancel") {
            onCancel();
          } else if (item.type === "submit") {
            onSubmit(selections);
          }
        }
        break;
      case " ":
        e.preventDefault();
        {
          const item = allItems[focusIdx];
          if (item.type === "option") {
            handleToggle(item.promptIdx, item.label);
          } else if (item.type === "submit" && allAnswered) {
            onSubmit(selections);
          }
        }
        break;
    }
  };

  let itemIdx = 0;

  return (
    <div className="w-full px-4 py-3">
      <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[40%] mx-auto">
        <div tabIndex={-1} className="rounded-2xl border border-neutral-700 bg-neutral-900 overflow-hidden" onKeyDown={handleKeyDown}>
          <div className="flex items-center gap-2 px-4 py-3 border-b border-neutral-700">
            <div className="text-blue-400 shrink-0">
              <HelpCircle className="w-5 h-5" />
            </div>
            <p className="text-sm font-medium text-neutral-100 truncate">
              {payload.prompts.length > 1 ? "Questions" : payload.prompts[0]?.header || "Question"}
            </p>
          </div>

          <div className="px-4 py-3 space-y-3">
            {payload.prompts.map((prompt, pi) => (
              <div key={pi}>
                {payload.prompts.length > 1 && (
                  <p className="text-xs font-medium text-neutral-400 mb-1">{prompt.header}</p>
                )}
                <p className="text-sm text-neutral-200 mb-2">{prompt.question}</p>
                <div className="space-y-1">
                  {prompt.options.map((opt) => {
                    const idx = itemIdx++;
                    const selected = selections[pi].includes(opt.label);
                    const focused = focusIdx === idx;
                    return (
                      <button
                        key={opt.label}
                        ref={(el) => { btnRefs.current[idx] = el; }}
                        tabIndex={focused ? 0 : -1}
                        onClick={() => handleToggle(pi, opt.label)}
                        className={`w-full text-left px-3 py-2 rounded-lg text-xs transition-colors outline-none
                          ${selected
                            ? "bg-blue-600 text-white"
                            : "bg-neutral-800 text-neutral-300 border border-neutral-700"
                          }
                          ${focused && !selected ? "ring-1 ring-blue-400" : ""}
                        `}
                      >
                        <span className="font-medium">{opt.label}</span>
                        {opt.description && (
                          <span className="block text-neutral-500 mt-0.5">{opt.description}</span>
                        )}
                      </button>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>

          <div className="flex items-center justify-end gap-2 px-4 py-2.5 border-t border-neutral-700">
            <button
              ref={(el) => { btnRefs.current[itemIdx] = el; }}
              tabIndex={focusIdx === itemIdx ? 0 : -1}
              onClick={onCancel}
              className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors border border-neutral-600 text-neutral-300 hover:bg-neutral-800 hover:text-neutral-100 outline-none ${
                focusIdx === itemIdx ? "ring-2 ring-neutral-400" : ""
              }`}
            >
              Cancel
            </button>
            {(() => { const idx = itemIdx++; return (
              <button
                ref={(el) => { btnRefs.current[idx] = el; }}
                tabIndex={focusIdx === idx ? 0 : -1}
                onClick={() => onSubmit(selections)}
                disabled={!allAnswered}
                className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors bg-blue-600 text-white hover:bg-blue-500 disabled:opacity-50 disabled:cursor-not-allowed outline-none ${
                  focusIdx === idx ? "ring-2 ring-blue-400" : ""
                }`}
              >
                Submit
              </button>
            ); })()}
          </div>
        </div>
      </div>
    </div>
  );
}
