import { useCallback, type ReactNode } from "react";

export interface AccordionItem {
  value: string;
  header: ReactNode;
  children: ReactNode;
  disabled?: boolean;
}

interface ReviewAccordionProps {
  items: AccordionItem[];
  open: string[];
  onOpenChange: (open: string[]) => void;
}

export function ReviewAccordion({ items, open, onOpenChange }: ReviewAccordionProps) {
  const isOpen = useCallback((value: string) => open.includes(value), [open]);

  const toggle = useCallback((value: string) => {
    if (isOpen(value)) {
      onOpenChange(open.filter((v) => v !== value));
    } else {
      onOpenChange([...open, value]);
    }
  }, [open, onOpenChange, isOpen]);

  return (
    <div className="review-accordion">
      {items.map((item) => (
        <div key={item.value} className="review-accordion-item" data-file={item.value}>
          <div
            className="review-accordion-header sticky top-0 z-10"
            onClick={() => {
              if (!item.disabled) toggle(item.value);
            }}
          >
            {item.header}
          </div>
          {isOpen(item.value) && !item.disabled && (
            <div className="review-accordion-content">
              {item.children}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
