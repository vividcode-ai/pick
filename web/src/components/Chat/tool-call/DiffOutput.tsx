import { DiffViewer } from "../DiffViewer";

interface DiffOutputProps {
  content: string;
}

export function DiffOutput({ content }: DiffOutputProps) {
  return <DiffViewer diffText={content} />;
}
