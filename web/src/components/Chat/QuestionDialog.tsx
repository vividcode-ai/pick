import { useState } from "react";
import { HelpCircle } from "lucide-react";
import type { QuestionPayload } from "../../types/events";

interface QuestionDialogProps {
  payload: QuestionPayload;
  onSubmit: (answers: string[][]) => void;
  onCancel: () => void;
}

export function QuestionDialog({ payload, onSubmit, onCancel }: QuestionDialogProps) {
  const [selections, setSelections] = useState<string[][]>(
    payload.prompts.map(() => [])
  );

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

  return (
    <div className="w-full px-4 py-3">
      <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[40%] mx-auto">
        <div className="rounded-2xl border border-neutral-700 bg-neutral-900 overflow-hidden">
          <div className="flex items-center gap-2 px-4 py-3 border-b border-neutral-700">
            <div className="text-blue-400 shrink-0">
              <HelpCircle className="w-5 h-5" />
            </div>
            <p className="text-sm font-medium text-neutral-100 truncate">
              {payload.prompts.length > 1 ? "Questions" : payload.prompts[0]?.header || "Question"}
            </p>
          </div>

          <div className="px-4 py-3 space-y-3 max-h-[200px] overflow-y-auto">
            {payload.prompts.map((prompt, pi) => (
              <div key={pi}>
                {payload.prompts.length > 1 && (
                  <p className="text-xs font-medium text-neutral-400 mb-1">{prompt.header}</p>
                )}
                <p className="text-sm text-neutral-200 mb-2">{prompt.question}</p>
                <div className="space-y-1">
                  {prompt.options.map((opt) => {
                    const selected = selections[pi].includes(opt.label);
                    return (
                      <button
                        key={opt.label}
                        onClick={() => handleToggle(pi, opt.label)}
                        className={`w-full text-left px-3 py-2 rounded-lg text-xs transition-colors ${
                          selected
                            ? "bg-blue-600/20 border border-blue-500/50 text-blue-200"
                            : "bg-neutral-800 border border-neutral-700 text-neutral-300 hover:bg-neutral-750"
                        }`}
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
              onClick={onCancel}
              className="px-3 py-1.5 rounded-lg border border-neutral-600 text-neutral-300 hover:bg-neutral-800 hover:text-neutral-100 transition-colors text-xs font-medium"
            >
              Cancel
            </button>
            <button
              onClick={() => onSubmit(selections)}
              disabled={!allAnswered}
              className="px-3 py-1.5 rounded-lg bg-blue-600 text-white hover:bg-blue-500 disabled:opacity-50 disabled:cursor-not-allowed transition-colors text-xs font-medium"
            >
              Submit
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
