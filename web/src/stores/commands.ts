import { useCallback, useState } from "react";

export interface Command {
  id: string;
  label: string;
  description?: string;
  category?: string;
  keywords?: string[];
  shortcut?: string;
  action: () => void;
}

let commands: Command[] = [];
const listeners = new Set<() => void>();

export function registerCommand(cmd: Command) {
  commands = [...commands, cmd];
  listeners.forEach((l) => l());
}

export function unregisterCommand(id: string) {
  commands = commands.filter((c) => c.id !== id);
  listeners.forEach((l) => l());
}

export function getCommands(): Command[] {
  return commands;
}

export function subscribeToCommands(callback: () => void) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

// Default commands
export function registerDefaultCommands(actions: {
  newSession: () => void;
  toggleSidebar: () => void;
  toggleTheme: () => void;
  openSettings: () => void;
}) {
  registerCommand({
    id: "new-session",
    label: "New Session",
    description: "Create a new chat session",
    category: "Session",
    keywords: ["new", "create", "chat"],
    shortcut: "Ctrl+N",
    action: actions.newSession,
  });

  registerCommand({
    id: "toggle-sidebar",
    label: "Toggle Sidebar",
    description: "Show or hide the sidebar",
    category: "View",
    keywords: ["sidebar", "panel", "toggle"],
    shortcut: "Ctrl+B",
    action: actions.toggleSidebar,
  });

  registerCommand({
    id: "toggle-theme",
    label: "Toggle Theme",
    description: "Switch between light, dark, and system theme",
    category: "View",
    keywords: ["theme", "dark", "light", "mode"],
    action: actions.toggleTheme,
  });

  registerCommand({
    id: "open-settings",
    label: "Open Settings",
    description: "Open the settings panel",
    category: "System",
    keywords: ["settings", "preferences", "config"],
    shortcut: "Ctrl+,",
    action: actions.openSettings,
  });
}
