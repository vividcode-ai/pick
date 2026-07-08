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
  approval_id: string;
  tool_name: string;
  tool_args: string;
  permission?: string;
  source?: "tool" | "permission_hook";
}

export interface QuestionOption {
  label: string;
  description: string;
}

export interface QuestionPromptPayload {
  question: string;
  header: string;
  options: QuestionOption[];
  multiple: boolean;
}

export interface QuestionPayload {
  question_id: string;
  prompts: QuestionPromptPayload[];
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
  extraMode?: "goal" | "loop";
}

export interface SessionInfo {
  id: string;
  messages: ChatMessage[];
  connected: boolean;
  streaming: boolean;
}

export interface ModelCapabilities {
  input?: string[];
  reasoning?: boolean;
}

export interface ModelInfo {
  id: string;
  name: string;
  reasoning: boolean;
  context?: number;
  cost_input?: number;
  cost_output?: number;
  capabilities?: ModelCapabilities;
  status?: "active" | "beta" | "alpha" | "deprecated";
  release_date?: string;
}

export interface FlatModel extends ModelInfo {
  provider: string;
  providerDisplayName: string;
  searchText: string;
}

export interface ProviderInfo {
  provider: string;
  has_key: boolean;
  models: ModelInfo[];
}

export interface ProvidersResponse {
  providers: ProviderInfo[];
  last_provider: string | null;
  last_model: string | null;
  thinking_level: string | null;
}

export interface GroupInfo<T> {
  category: string;
  items: T[];
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

export interface LineComment {
  id: string;
  file: string;
  line: number;
  line_end?: number;
  side?: "additions" | "deletions";
  comment: string;
  time: number;
  resolved: boolean;
}

export interface GitDiffEntry {
  path: string;
  status: string;
  additions: number;
  deletions: number;
  patch: string;
  binary: boolean;
}

export interface GitDiffsResponse {
  branch: string;
  files: GitDiffEntry[];
}

export interface ReviewFileComment {
  file: string;
  line: number;
  line_end?: number;
  side?: "additions" | "deletions";
  comment: string;
}
