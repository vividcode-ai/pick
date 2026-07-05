import { useEffect, useRef, useState, useCallback } from "react";

export function useLineHover(containerRef: React.RefObject<HTMLDivElement | null>) {
  const [hoveredLine, setHoveredLine] = useState<number | null>(null);
  const [hoveredRect, setHoveredRect] = useState<DOMRect | null>(null);
  const currentLineRef = useRef<number | null>(null);
  const containerRectRef = useRef<DOMRect | null>(null);

  const handleMouseMove = useCallback((e: MouseEvent) => {
    const container = containerRef.current;
    if (!container) return;

    const lineNums = container.querySelectorAll<HTMLElement>(".line-num");
    if (!lineNums.length) {
      if (currentLineRef.current !== null) {
        currentLineRef.current = null;
        setHoveredLine(null);
        setHoveredRect(null);
      }
      return;
    }

    let found = false;
    for (const el of lineNums) {
      const rect = el.getBoundingClientRect();
      if (e.clientX >= rect.left && e.clientX <= rect.right &&
          e.clientY >= rect.top && e.clientY <= rect.bottom) {
        const line = parseInt(el.textContent || "", 10);
        if (!isNaN(line) && line !== currentLineRef.current) {
          currentLineRef.current = line;
          setHoveredLine(line);
          setHoveredRect(container.getBoundingClientRect());
        }
        found = true;
        break;
      }
    }
    if (!found && currentLineRef.current !== null) {
      currentLineRef.current = null;
      setHoveredLine(null);
      setHoveredRect(null);
    }
  }, [containerRef]);

  const handleMouseLeave = useCallback(() => {
    currentLineRef.current = null;
    setHoveredLine(null);
    setHoveredRect(null);
  }, []);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    container.addEventListener("mousemove", handleMouseMove);
    container.addEventListener("mouseleave", handleMouseLeave);
    return () => {
      container.removeEventListener("mousemove", handleMouseMove);
      container.removeEventListener("mouseleave", handleMouseLeave);
    };
  }, [containerRef, handleMouseMove, handleMouseLeave]);

  return { hoveredLine, hoveredRect };
}
