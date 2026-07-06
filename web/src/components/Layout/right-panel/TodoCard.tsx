import type { TodoItem } from "../../../types/events";

interface TodoCardProps {
  todos: TodoItem[];
}

const statusConfig: Record<string, { icon: string; color: string }> = {
  pending: { icon: "○", color: "text-neutral-400" },
  in_progress: { icon: "◉", color: "text-blue-400" },
  completed: { icon: "✔", color: "text-green-400" },
  cancelled: { icon: "✕", color: "text-red-400" },
};

const priorityColor: Record<string, string> = {
  high: "bg-red-500/10 text-red-400",
  medium: "bg-yellow-500/10 text-yellow-400",
  low: "bg-blue-500/10 text-blue-400",
};

export function TodoCard({ todos }: TodoCardProps) {
  if (todos.length === 0) return null;

  const sorted = [...todos].sort((a, b) => {
    const order = { high: 0, medium: 1, low: 2 };
    return (order[a.priority] ?? 1) - (order[b.priority] ?? 1);
  });

  return (
    <div className="rounded-xl border border-[var(--border-base)] bg-[var(--surface-secondary)] shadow-sm overflow-hidden">
      <div className="px-3 py-2.5 border-b border-[var(--border-base)]">
        <h3 className="text-xs font-semibold text-[var(--text-primary)] uppercase tracking-wider">
          Todo Tools
        </h3>
      </div>
      <div className="p-2 space-y-1">
        {sorted.map((item, i) => {
          const cfg = statusConfig[item.status] ?? statusConfig.pending;
          return (
            <div
              key={i}
              className="flex items-start gap-2 px-2 py-1.5 rounded-lg hover:bg-[var(--surface-hover)] transition-colors"
            >
              <span className={`mt-0.5 text-sm flex-shrink-0 ${cfg.color}`}>
                {cfg.icon}
              </span>
              <span className="text-xs text-[var(--text-primary)] flex-1 leading-relaxed">
                {item.content}
              </span>
              <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium flex-shrink-0 ${priorityColor[item.priority] ?? ""}`}>
                {item.priority}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
