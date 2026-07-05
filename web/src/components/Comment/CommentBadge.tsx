import { MessageSquare, Plus } from "lucide-react";

interface CommentBadgeProps {
  line: number;
  hasComments: boolean;
  isHovered: boolean;
  onClick: () => void;
}

export function CommentBadge({ line, hasComments, isHovered, onClick }: CommentBadgeProps) {
  const visible = isHovered || hasComments;

  return (
    <button
      onClick={(e) => { e.stopPropagation(); onClick(); }}
      className={`absolute left-0 top-0 z-10 flex items-center justify-center w-5 h-full transition-all ${
        visible ? "opacity-100" : "opacity-0 pointer-events-none"
      } ${hasComments
        ? "text-blue-400 hover:text-blue-300"
        : "text-[var(--text-muted)] hover:text-[var(--accent-primary)]"
      }`}
      style={{ transform: "translateX(-20px)" }}
      title={hasComments ? `View comments on line ${line}` : `Add comment on line ${line}`}
    >
      {hasComments
        ? <MessageSquare className="w-3 h-3" />
        : <Plus className="w-3.5 h-3.5" />
      }
    </button>
  );
}
