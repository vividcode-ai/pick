import { useRef, useEffect, useSyncExternalStore } from "react";
import type { ChatMessage } from "../../types/events";
import { MessageBubble } from "./MessageBubble";
import { getAppSettings, subscribeAppSettings } from "../../stores/appSettings";

interface ChatViewProps {
  messages: ChatMessage[];
  onFork?: (message: ChatMessage) => void;
}

function useAppSettings() {
  return useSyncExternalStore(subscribeAppSettings, getAppSettings, getAppSettings);
}

export function ChatView({ messages, onFork }: ChatViewProps) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const userScrolledAwayRef = useRef(false);
  const settings = useAppSettings();

  // Track user scroll position. If they scroll up, stop auto-scrolling.
  // Resume once they scroll back to the bottom.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const onScroll = () => {
      const threshold = 100;
      const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < threshold;
      userScrolledAwayRef.current = !atBottom;
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, []);

  useEffect(() => {
    if (!userScrolledAwayRef.current) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages]);

  const visibleMessages = messages.filter((msg) => {
    if (msg.role === "tool" && msg.toolCall?.name === "todo_plan") return false;
    if (msg.role === "tool" && !settings.show_tool_calls) return false;
    if (msg.role === "thinking" && !settings.show_thinking) return false;
    return true;
  });

  return (
    <div ref={containerRef} className="flex-1 overflow-y-auto min-h-0">
      <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[40%] mx-auto px-4 py-4 space-y-3">
        {visibleMessages.map((msg) => (
          <MessageBubble key={msg.id + msg.timestamp} message={msg} onFork={msg.role === "assistant" ? () => onFork?.(msg) : undefined} />
        ))}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
