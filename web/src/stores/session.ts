import type { ChatMessage } from "../types/events";

export interface SessionStore {
  sessionId: string | null;
  messages: ChatMessage[];
  streaming: boolean;
  connected: boolean;
}
