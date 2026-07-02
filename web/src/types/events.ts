export interface WsMessage {
  type: string;
  payload?: any;
}

export interface MessageUpdatePayload {
  text: string;
  thinking?: string;
  delta: boolean;
}

export interface ThinkingPayload {
  text: string;
}

export interface ToolStartPayload {
  tool_call_id: string;
  tool_name: string;
  args: Record<string, any>;
}

export interface ToolUpdatePayload {
  tool_call_id: string;
  partial_output: string;
}

export interface ToolEndPayload {
  tool_call_id: string;
  tool_name: string;
  output: string;
  is_error: boolean;
}

export interface UsagePayload {
  input: number;
  output: number;
}

export interface AgentEndPayload {
  usage: UsagePayload;
}

export interface ApprovalRequiredPayload {
  id: string;
  tool_name: string;
  tool_args: string;
  permission: string;
}

export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system" | "tool" | "thinking";
  content: string;
  toolCall?: {
    name: string;
    args: Record<string, any>;
    output?: string;
    isError?: boolean;
    isStreaming?: boolean;
  };
  timestamp: number;
}

export interface SessionInfo {
  id: string;
  messages: ChatMessage[];
  connected: boolean;
  streaming: boolean;
}

export interface ModelInfo {
  id: string;
  name: string;
  reasoning: boolean;
}

export interface ProviderInfo {
  provider: string;
  has_key: boolean;
  models: ModelInfo[];
}

export interface TodoItem {
  content: string;
  status: "pending" | "in_progress" | "completed" | "cancelled";
  priority: "high" | "medium" | "low";
}

export interface GitChange {
  path: string;
  status: string;
}

export interface GitInfo {
  branch: string;
  changes: GitChange[];
  cwd: string;
}
