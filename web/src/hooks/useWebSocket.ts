import { useCallback, useEffect, useRef, useState } from "react";
import type {
  WsMessage,
  ChatMessage,
  ToolStartPayload,
  ToolEndPayload,
  MessageUpdatePayload,
} from "../types/events";

interface UseWebSocketOptions {
  url: string;
  onMessage?: (msg: WsMessage) => void;
  onError?: (error: string) => void;
  autoReconnect?: boolean;
}

export function useWebSocket({
  url,
  onMessage,
  onError,
  autoReconnect = true,
}: UseWebSocketOptions) {
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [connected, setConnected] = useState(false);
  const reconnectAttemptRef = useRef(0);

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) return;

    try {
      const ws = new WebSocket(url);

      ws.onopen = () => {
        setConnected(true);
        reconnectAttemptRef.current = 0;
      };

      ws.onclose = () => {
        setConnected(false);
        wsRef.current = null;
        if (autoReconnect && reconnectAttemptRef.current < 5) {
          const delay = Math.min(1000 * 2 ** reconnectAttemptRef.current, 10000);
          reconnectAttemptRef.current++;
          reconnectTimerRef.current = setTimeout(connect, delay);
        }
      };

      ws.onerror = () => {
        onError?.("WebSocket connection error");
      };

      ws.onmessage = (event) => {
        try {
          const msg: WsMessage = JSON.parse(event.data);
          onMessage?.(msg);
        } catch (e) {
          console.error("Failed to parse WS message:", e);
        }
      };

      wsRef.current = ws;
    } catch (e) {
      onError?.(`Failed to connect: ${e}`);
    }
  }, [url, onMessage, onError, autoReconnect]);

  const disconnect = useCallback(() => {
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
    wsRef.current?.close();
    wsRef.current = null;
    setConnected(false);
  }, []);

  const send = useCallback((msg: WsMessage) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(msg));
    }
  }, []);

  useEffect(() => {
    return () => {
      disconnect();
    };
  }, [disconnect]);

  return { connected, connect, disconnect, send };
}

export function useAgentSession(wsUrl: string) {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [connected, setConnected] = useState(false);

  const handleMessage = useCallback((msg: WsMessage) => {
    switch (msg.type) {
      case "session_created": {
        setSessionId(msg.payload.session_id);
        setMessages([]);
        break;
      }
      case "message_update": {
        const payload = msg.payload as MessageUpdatePayload;
        setMessages((prev) => {
          const last = prev[prev.length - 1];
          if (last?.role === "assistant" && last.content) {
            const updated = [...prev];
            updated[updated.length - 1] = {
              ...last,
              content: last.content + payload.text,
            };
            return updated;
          }
          return [
            ...prev,
            {
              id: crypto.randomUUID(),
              role: "assistant",
              content: payload.text,
              timestamp: Date.now(),
            },
          ];
        });
        break;
      }
      case "tool_start": {
        const payload = msg.payload as ToolStartPayload;
        setMessages((prev) => [
          ...prev,
          {
            id: payload.tool_call_id,
            role: "tool",
            content: "",
            toolCall: {
              name: payload.tool_name,
              args: payload.args,
              isStreaming: true,
            },
            timestamp: Date.now(),
          },
        ]);
        break;
      }
      case "tool_update": {
        const { tool_call_id, partial_output } = msg.payload;
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
        const payload = msg.payload as ToolEndPayload;
        setMessages((prev) =>
          prev.map((m) =>
            m.id === payload.tool_call_id && m.toolCall
              ? {
                  ...m,
                  content: payload.output,
                  toolCall: {
                    ...m.toolCall,
                    output: payload.output,
                    isError: payload.is_error,
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
            content: "Error: " + (msg.payload?.message || "Unknown error"),
            timestamp: Date.now(),
          },
        ]);
        break;
    }
  }, []);

  const ws = useWebSocket({
    url: wsUrl,
    onMessage: handleMessage,
  });

  useEffect(() => {
    setConnected(ws.connected);
  }, [ws.connected]);

  const createSession = useCallback(
    (modelId?: string, provider?: string) => {
      ws.send({
        type: "create_session",
        payload: { model_id: modelId, provider },
      });
    },
    [ws.send]
  );

  const ask = useCallback(
    (prompt: string) => {
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
      ws.send({
        type: "ask",
        payload: { prompt, session_id: sessionId },
      });
    },
    [ws.send, sessionId]
  );

  const cancel = useCallback(() => {
    ws.send({ type: "cancel" });
  }, [ws.send]);

  return {
    sessionId,
    messages,
    streaming,
    connected,
    createSession,
    ask,
    cancel,
    connect: ws.connect,
    disconnect: ws.disconnect,
  };
}
