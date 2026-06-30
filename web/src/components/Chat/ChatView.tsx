import { useRef, useEffect } from "react";
import type { ChatMessage } from "../../types/events";
import { MessageBubble } from "./MessageBubble";

interface ChatViewProps {
  messages: ChatMessage[];
  streaming: boolean;
}

export function ChatView({ messages, streaming }: ChatViewProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  return (
    <div className="flex-1 overflow-y-auto min-h-0">
      <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[50%] mx-auto px-4 py-4 space-y-3">
        {messages.map((msg) => (
          <MessageBubble key={msg.id + msg.timestamp} message={msg} />
        ))}
        {streaming && (
          <div className="flex items-center gap-2 text-neutral-400 px-4 py-2">
            <span className="w-2 h-2 bg-neutral-400 rounded-full animate-pulse" />
            <span className="text-sm">Thinking...</span>
          </div>
        )}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
