const STORAGE_KEY_PREFIX = "pick_review_scroll_";

export function saveReviewScroll(sessionId: string, scrollTop: number) {
  try {
    sessionStorage.setItem(STORAGE_KEY_PREFIX + sessionId, String(scrollTop));
  } catch {}
}

export function getReviewScroll(sessionId: string): number | null {
  try {
    const val = sessionStorage.getItem(STORAGE_KEY_PREFIX + sessionId);
    return val ? parseInt(val, 10) : null;
  } catch {
    return null;
  }
}

export function clearReviewScroll(sessionId: string) {
  try {
    sessionStorage.removeItem(STORAGE_KEY_PREFIX + sessionId);
  } catch {}
}

// Debounced wrapper for saveReviewScroll
export function createScrollSaver(sessionId: string): (scrollTop: number) => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return (scrollTop: number) => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      saveReviewScroll(sessionId, scrollTop);
      timer = null;
    }, 500);
  };
}

// IntersectionObserver-based visibility tracker
export function createVisibilityTracker(
  container: HTMLElement,
  margin: number = 300,
): { observe: (el: HTMLElement, file: string) => void; unobserve: (file: string) => void; visible: Set<string>; destroy: () => void } {
  const visible = new Set<string>();
  const fileToEl = new Map<string, HTMLElement>();
  let observer: IntersectionObserver | null = null;

  const createObserver = () => {
    if (observer) observer.disconnect();
    observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const file = entry.target.getAttribute("data-file");
          if (!file) continue;
          if (entry.isIntersecting) {
            visible.add(file);
          } else {
            visible.delete(file);
          }
        }
      },
      {
        root: container,
        rootMargin: `${margin}px 0px`,
      },
    );
    // Re-observe all existing elements
    for (const [file, el] of fileToEl) {
      observer.observe(el);
    }
  };

  createObserver();

  return {
    observe(el: HTMLElement, file: string) {
      fileToEl.set(file, el);
      // Mark as visible by default (will be corrected on first intersection)
      visible.add(file);
      if (observer) observer.observe(el);
    },
    unobserve(file: string) {
      const el = fileToEl.get(file);
      if (el && observer) observer.unobserve(el);
      fileToEl.delete(file);
      visible.delete(file);
    },
    visible,
    destroy() {
      if (observer) observer.disconnect();
      fileToEl.clear();
      visible.clear();
    },
  };
}
