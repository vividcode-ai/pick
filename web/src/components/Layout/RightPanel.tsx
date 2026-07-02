import type { GitInfo, TodoItem } from "../../types/events";
import { StatusCard } from "./right-panel/StatusCard";
import { TodoCard } from "./right-panel/TodoCard";

interface RightPanelProps {
  diffs?: { filePath: string; content: string }[];
  connected: boolean;
  sessionId: string | null;
  todos: TodoItem[];
  gitInfo: GitInfo | null;
  baseUrl: string;
  onCommitRequest: (message: string) => void;
}

export function RightPanel({ sessionId, todos, gitInfo, baseUrl, onCommitRequest }: RightPanelProps) {
  return (
    <div className="flex flex-col gap-3 p-3 h-full min-h-0 overflow-y-auto">
      <StatusCard
        gitInfo={gitInfo}
        sessionId={sessionId}
        baseUrl={baseUrl}
        onCommitRequest={onCommitRequest}
      />
      <TodoCard todos={todos} />
    </div>
  );
}
