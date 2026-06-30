import { useState, useRef, type KeyboardEvent } from "react";
import type { ProviderInfo } from "../../types/events";

export interface ChatInputHandle {
  focus: () => void;
}

interface ChatInputProps {
  onSend: (text: string) => void;
  disabled: boolean;
  onCancel?: () => void;
  connected: boolean;
  providers: ProviderInfo[];
  selectedProvider: string;
  onProviderChange: (p: string) => void;
  selectedModel: string;
  onModelChange: (m: string) => void;
  modelsForProvider: { id: string; name: string; reasoning: boolean }[];
  selectedModelDetail: { id: string; name: string; reasoning: boolean } | null;
  thinkingLevel: string;
  onThinkingLevelChange: (l: string) => void;
}

const THINKING_LEVELS = [
  { value: "off", label: "Off" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
];

export function ChatInput({
  onSend,
  disabled,
  onCancel,
  connected,
  providers,
  selectedProvider,
  onProviderChange,
  selectedModel,
  onModelChange,
  modelsForProvider,
  selectedModelDetail,
  thinkingLevel,
  onThinkingLevelChange,
}: ChatInputProps) {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const handleSend = () => {
    const trimmed = input.trim();
    if (!trimmed || disabled) return;
    onSend(trimmed);
    setInput("");
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
    if (e.key === "Escape" && disabled && onCancel) {
      onCancel();
    }
  };

  const handleInput = () => {
    const el = textareaRef.current;
    if (el) {
      el.style.height = "auto";
      el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
    }
  };

  const insertCommand = (cmd: string) => {
    const newVal = cmd + " ";
    setInput(newVal);
    if (textareaRef.current) {
      textareaRef.current.focus();
      const len = newVal.length;
      textareaRef.current.setSelectionRange(len, len);
      handleInput();
    }
  };

  return (
    <div className="w-full px-4 py-3">
      <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[50%] mx-auto rounded-2xl border border-neutral-700 bg-neutral-800">
          {/* Top: textarea */}
          <textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            onInput={handleInput}
            placeholder={
              connected ? "Type a message..." : "Connecting..."
            }
            rows={1}
            disabled={!connected}
            className="w-full bg-transparent text-neutral-100 px-4 pt-3 pb-3 text-sm resize-none outline-none placeholder-neutral-500 disabled:opacity-50 min-h-[44px]"
          />

          {/* Bottom: controls */}
          <div className="flex items-center justify-between px-3 pb-3 pt-1.5 border-t border-neutral-700/50">
            {/* Left: command buttons */}
            <div className="flex items-center gap-1">
              <button
                onClick={() => insertCommand("/build")}
                disabled={disabled || !connected}
                className="px-2.5 py-1 text-xs rounded-md bg-neutral-700 hover:bg-neutral-600 text-neutral-300 disabled:opacity-40 transition-colors"
              >
                Build
              </button>
              <button
                onClick={() => insertCommand("/plan")}
                disabled={disabled || !connected}
                className="px-2.5 py-1 text-xs rounded-md bg-neutral-700 hover:bg-neutral-600 text-neutral-300 disabled:opacity-40 transition-colors"
              >
                Plan
              </button>
            </div>

            {/* Right: model selectors + send */}
            <div className="flex items-center gap-2">
              <select
                value={selectedProvider}
                onChange={(e) => onProviderChange(e.target.value)}
                disabled={disabled}
                className="bg-neutral-700 text-neutral-200 text-xs rounded-md px-2 py-1 border border-neutral-600 outline-none disabled:opacity-40"
              >
                {providers.map((p) => (
                  <option key={p.provider} value={p.provider}>
                    {p.provider}
                  </option>
                ))}
              </select>

              <select
                value={selectedModel}
                onChange={(e) => onModelChange(e.target.value)}
                disabled={disabled}
                className="bg-neutral-700 text-neutral-200 text-xs rounded-md px-2 py-1 border border-neutral-600 outline-none max-w-[120px] disabled:opacity-40"
              >
                {modelsForProvider.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name}
                  </option>
                ))}
              </select>

              {selectedModelDetail?.reasoning && (
                <select
                  value={thinkingLevel}
                  onChange={(e) => onThinkingLevelChange(e.target.value)}
                  disabled={disabled}
                  className="bg-neutral-700 text-neutral-200 text-xs rounded-md px-2 py-1 border border-neutral-600 outline-none disabled:opacity-40"
                >
                  {THINKING_LEVELS.map((l) => (
                    <option key={l.value} value={l.value}>
                      {l.label}
                    </option>
                  ))}
                </select>
              )}

              {disabled ? (
                <button
                  onClick={onCancel}
                  className="px-3 py-1.5 bg-red-600 text-white rounded-lg text-xs hover:bg-red-700 transition-colors"
                >
                  Stop
                </button>
              ) : (
                <button
                  onClick={handleSend}
                  disabled={!input.trim() || !connected}
                  className="px-3 py-1.5 bg-blue-600 text-white rounded-lg text-xs hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  Send
                </button>
              )}
            </div>
          </div>
        </div>
    </div>
  );
}
