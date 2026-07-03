import type { GitInfo, TodoItem } from "../../types/events";
import { StatusCard } from "./right-panel/StatusCard";
import { TodoCard } from "./right-panel/TodoCard";

interface RightPanelProps {
  diffs?: { filePath: string; content: string }[];
  connected: boolean;
  sessionId: string | null;
  todos: TodoItem[];
  gitInfo: GitInfo | null;
  onCommitRequest: (message: string) => void;
}

export function RightPanel({ sessionId, todos, gitInfo, onCommitRequest }: RightPanelProps) {
  return (
    <>
      <StatusCard
        gitInfo={gitInfo}
        sessionId={sessionId}
        onCommitRequest={onCommitRequest}
      />
      <TodoCard todos={todos} />
    </>
  );
}
