import { useState, useRef, type KeyboardEvent } from "react";

interface ChatInputProps {
  onSend: (text: string) => void;
  disabled: boolean;
  onCancel?: () => void;
  connected: boolean;
}

export function ChatInput({
  onSend,
  disabled,
  onCancel,
  connected,
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
      el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
    }
  };

  return (
    <div className="border-t border-neutral-800 bg-neutral-900 px-4 py-3">
      <div className="flex items-end gap-2 max-w-4xl mx-auto">
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
          className="flex-1 bg-neutral-800 text-neutral-100 rounded-lg px-3 py-2 text-sm resize-none outline-none focus:ring-1 focus:ring-blue-500 placeholder-neutral-500 disabled:opacity-50"
        />
        {disabled ? (
          <button
            onClick={onCancel}
            className="px-3 py-2 bg-red-600 text-white rounded-lg text-sm hover:bg-red-700"
          >
            Stop
          </button>
        ) : (
          <button
            onClick={handleSend}
            disabled={!input.trim() || !connected}
            className="px-3 py-2 bg-blue-600 text-white rounded-lg text-sm hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Send
          </button>
        )}
      </div>
    </div>
  );
}
