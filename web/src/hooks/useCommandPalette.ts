import { useEffect, useState, useCallback, useSyncExternalStore } from "react";
import { subscribeToCommands, getCommands, type Command } from "../stores/commands";

export function useCommandPalette() {
  const [open, setOpen] = useState(false);
  const commands = useSyncExternalStore(subscribeToCommands, getCommands, getCommands);

  const toggle = useCallback(() => setOpen((v) => !v), []);
  const close = useCallback(() => setOpen(false), []);
  const openPalette = useCallback(() => setOpen(true), []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        toggle();
      }
      if (e.key === "Escape" && open) {
        close();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, toggle, close]);

  return { open, close, toggle, openPalette, commands };
}
