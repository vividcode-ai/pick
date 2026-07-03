import { useRef, useEffect } from "react";
import type { ChatMessage } from "../../types/events";
import { MessageBubble } from "./MessageBubble";

interface ChatViewProps {
  messages: ChatMessage[];
  onFork?: (message: ChatMessage) => void;
}

export function ChatView({ messages, onFork }: ChatViewProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  return (
    <div className="flex-1 overflow-y-auto min-h-0">
      <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[40%] mx-auto px-4 py-4 space-y-3">
        {messages.filter((msg) => !(msg.role === "tool" && msg.toolCall?.name === "todo_plan")).map((msg) => (
          <MessageBubble key={msg.id + msg.timestamp} message={msg} onFork={msg.role === "assistant" ? () => onFork?.(msg) : undefined} />
        ))}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
