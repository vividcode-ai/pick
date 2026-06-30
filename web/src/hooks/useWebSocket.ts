import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ChatMessage,
  ToolStartPayload,
  ToolEndPayload,
  MessageUpdatePayload,
} from "../types/events";

export function useAgentSession(baseUrl: string) {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [connected, setConnected] = useState(false);
  const esRef = useRef<EventSource | null>(null);
  const sessionIdRef = useRef<string | null>(null);

  const handleEvent = useCallback((type: string, rawData: string) => {
    let payload: any;
    try {
      payload = JSON.parse(rawData);
    } catch {
      return;
    }

    switch (type) {
      case "message_update": {
        const p = payload as MessageUpdatePayload;
        setMessages((prev) => {
          const last = prev[prev.length - 1];
          if (last?.role === "assistant" && last.content) {
            const updated = [...prev];
            updated[updated.length - 1] = {
              ...last,
              content: p.text,
            };
            return updated;
          }
          return [
            ...prev,
            {
              id: crypto.randomUUID(),
              role: "assistant",
              content: p.text,
              timestamp: Date.now(),
            },
          ];
        });
        break;
      }
      case "tool_start": {
        const p = payload as ToolStartPayload;
        setMessages((prev) => [
          ...prev,
          {
            id: p.tool_call_id,
            role: "tool",
            content: "",
            toolCall: {
              name: p.tool_name,
              args: p.args,
              isStreaming: true,
            },
            timestamp: Date.now(),
          },
        ]);
        break;
      }
      case "tool_update": {
        const { tool_call_id, partial_output } = payload;
        setMessages((prev) =>
          prev.map((m) =>
            m.id === tool_call_id && m.toolCall
              ? {
                  ...m,
                  content: m.content + partial_output,
                }
              : m
          )
        );
        break;
      }
      case "tool_end": {
        const p = payload as ToolEndPayload;
        setMessages((prev) =>
          prev.map((m) =>
            m.id === p.tool_call_id && m.toolCall
              ? {
                  ...m,
                  content: p.output,
                  toolCall: {
                    ...m.toolCall,
                    output: p.output,
                    isError: p.is_error,
                    isStreaming: false,
                  },
                }
              : m
          )
        );
        break;
      }
      case "agent_end": {
        setStreaming(false);
        break;
      }
      case "turn_end":
        break;
      case "error":
        setStreaming(false);
        setMessages((prev) => [
          ...prev,
          {
            id: crypto.randomUUID(),
            role: "system",
            content: "Error: " + (payload?.message || "Unknown error"),
            timestamp: Date.now(),
          },
        ]);
        break;
    }
  }, []);

  const connectSse = useCallback(
    (sid: string) => {
      if (esRef.current) {
        esRef.current.close();
      }

      const es = new EventSource(`${baseUrl}/events/${sid}`);

      es.onopen = () => {
        setConnected(true);
      };

      es.onerror = () => {
        setConnected(false);
      };

      const onEvent = (type: string) => (e: MessageEvent) =>
        handleEvent(type, e.data);

      es.addEventListener("message_update", onEvent("message_update"));
      es.addEventListener("tool_start", onEvent("tool_start"));
      es.addEventListener("tool_update", onEvent("tool_update"));
      es.addEventListener("tool_end", onEvent("tool_end"));
      es.addEventListener("agent_end", onEvent("agent_end"));
      es.addEventListener("turn_end", onEvent("turn_end"));
      es.addEventListener("error", onEvent("error"));
      es.addEventListener("approval_required", onEvent("approval_required"));
      es.addEventListener("question", onEvent("question"));

      esRef.current = es;
    },
    [baseUrl, handleEvent]
  );

  const createSession = useCallback(
    async (modelId?: string, provider?: string) => {
      const resp = await fetch(`${baseUrl}/sessions`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ model_id: modelId, provider }),
      });
      const data = await resp.json();
      const sid: string = data.session_id;
      sessionIdRef.current = sid;
      setSessionId(sid);
      setMessages([]);
      connectSse(sid);
    },
    [baseUrl, connectSse]
  );

  const ask = useCallback(
    async (prompt: string) => {
      const sid = sessionIdRef.current;
      if (!sid) return;

      setStreaming(true);
      setMessages((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          role: "user",
          content: prompt,
          timestamp: Date.now(),
        },
      ]);

      await fetch(`${baseUrl}/ask`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ session_id: sid, prompt }),
      });
    },
    [baseUrl]
  );

  const cancel = useCallback(async () => {
    const sid = sessionIdRef.current;
    if (!sid) return;
    await fetch(`${baseUrl}/cancel`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ session_id: sid }),
    });
  }, [baseUrl]);

  useEffect(() => {
    return () => {
      esRef.current?.close();
    };
  }, []);

  return {
    sessionId,
    messages,
    streaming,
    connected,
    createSession,
    ask,
    cancel,
    connect: () => {}, // No-op: sessions created explicitly
    disconnect: () => {
      esRef.current?.close();
    },
  };
}
