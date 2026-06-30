import { Markdown } from "../Markdown";

interface MarkdownOutputProps {
  content: string;
}

export function MarkdownOutput({ content }: MarkdownOutputProps) {
  return <Markdown content={content} />;
}
