interface BashOutputProps {
  content: string;
}

export function BashOutput({ content }: BashOutputProps) {
  return (
    <pre className="text-sm text-neutral-300 overflow-x-auto max-h-60 leading-relaxed">
      {content}
    </pre>
  );
}
