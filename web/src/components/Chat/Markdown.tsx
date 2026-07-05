import { useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Components } from "react-markdown";

interface MarkdownProps {
  content: string;
  className?: string;
}

function CodeBlock({ children, className }: { children?: React.ReactNode; className?: string }) {
  const code = useMemo(() => {
    if (typeof children === "string") return children;
    return "";
  }, [children]);

  const handleCopy = () => {
    navigator.clipboard.writeText(code);
  };

  return (
    <div className="relative group">
      <button
        onClick={handleCopy}
        className="absolute top-2 right-2 px-2 py-0.5 text-[10px] rounded bg-neutral-700 text-neutral-400 opacity-0 group-hover:opacity-100 transition-opacity hover:text-neutral-200"
      >
        Copy
      </button>
      <code className={className}>{children}</code>
    </div>
  );
}

const components: Components = {
  code({ className, children, ...props }) {
    const match = /language-(\w+)/.exec(className || "");
    const isInline = !match;
    if (isInline) {
      return <code className={className} {...props}>{children}</code>;
    }
    return <CodeBlock className={className}>{children}</CodeBlock>;
  },
};

export function Markdown({ content, className }: MarkdownProps) {
  return (
    <div className={`markdown-body ${className ?? ""}`}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={components}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
