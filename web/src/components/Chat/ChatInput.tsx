import { useState, useRef, useEffect, type KeyboardEvent } from "react";
import type { ProviderInfo } from "../../types/events";
import { ModelSelector } from "./ModelSelector";
import { ThinkingSelector } from "./ThinkingSelector";

interface ChatInputProps {
  onSend: (text: string) => void;
  disabled: boolean;
  onCancel?: () => void;
  connected: boolean;
  providers: ProviderInfo[];
  selectedModel: string;
  selectedProvider?: string;
  onModelChange: (modelId: string, provider: string) => void;
  thinkingLevel: string;
  onThinkingLevelChange: (l: string) => void;
}

export function ChatInput({
  onSend,
  disabled,
  onCancel,
  connected,
  providers,
  selectedModel,
  selectedProvider,
  onModelChange,
  thinkingLevel,
  onThinkingLevelChange,
}: ChatInputProps) {
  const [input, setInput] = useState("");
  const [currentCommand, setCurrentCommand] = useState<"build" | "plan">("build");
  const [commandOpen, setCommandOpen] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const commandRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (commandRef.current && !commandRef.current.contains(e.target as Node)) {
        setCommandOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

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

  const executeCommand = (cmd: "build" | "plan") => {
    insertCommand(`/${cmd}`);
    setCurrentCommand(cmd);
    setCommandOpen(false);
  };

  const handleCommandKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Tab") {
      e.preventDefault();
      setCurrentCommand((prev) => (prev === "build" ? "plan" : "build"));
    }
    if (e.key === "Escape") {
      setCommandOpen(false);
    }
  };

  return (
    <div className="w-full px-4 py-3">
      <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[40%] mx-auto rounded-2xl border border-neutral-700 bg-neutral-800">
        {/* Top: textarea */}
        <textarea
          ref={textareaRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          onInput={handleInput}
          placeholder={connected ? "Type a message..." : "Connecting..."}
          rows={1}
          disabled={!connected}
          className="w-full bg-transparent text-neutral-100 px-4 pt-3 pb-3 text-sm resize-none outline-none placeholder-neutral-500 disabled:opacity-50 min-h-[44px]"
        />

        {/* Bottom: controls */}
        <div className="flex items-center justify-between px-3 pb-3 pt-1.5 border-t border-neutral-700/50">
          {/* Left: command dropdown */}
          <div className="relative" ref={commandRef}>
            <div className="flex">
              <button
                onClick={() => insertCommand(`/${currentCommand}`)}
                disabled={disabled || !connected}
                onKeyDown={handleCommandKeyDown}
                className="px-2.5 py-1 text-xs rounded-l-md bg-neutral-700 hover:bg-neutral-600 text-neutral-300 disabled:opacity-40 transition-colors"
              >
                {currentCommand.charAt(0).toUpperCase() + currentCommand.slice(1)}
              </button>
              <button
                onClick={() => setCommandOpen((v) => !v)}
                disabled={disabled || !connected}
                className="px-1 py-1 text-xs rounded-r-md bg-neutral-700 hover:bg-neutral-600 text-neutral-400 border-l border-neutral-600 disabled:opacity-40 transition-colors"
              >
                <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                </svg>
              </button>
            </div>
            {commandOpen && (
              <div className="absolute bottom-full left-0 mb-1 w-24 rounded-md bg-neutral-800 border border-neutral-700 shadow-lg z-50 overflow-hidden">
                <button
                  onClick={() => executeCommand("build")}
                  className="w-full px-3 py-1.5 text-xs text-left text-neutral-300 hover:bg-neutral-700 transition-colors"
                >
                  Build
                </button>
                <button
                  onClick={() => executeCommand("plan")}
                  className="w-full px-3 py-1.5 text-xs text-left text-neutral-300 hover:bg-neutral-700 transition-colors"
                >
                  Plan
                </button>
              </div>
            )}
          </div>

          {/* Right: model selectors + send */}
          <div className="flex items-center gap-2">
            <ModelSelector
              providers={providers}
              selectedModel={selectedModel}
              selectedProvider={selectedProvider}
              onModelChange={onModelChange}
              disabled={disabled || !connected}
            />

            <ThinkingSelector
              providers={providers}
              selectedModel={selectedModel}
              thinkingLevel={thinkingLevel}
              onThinkingLevelChange={onThinkingLevelChange}
              disabled={disabled}
            />

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
