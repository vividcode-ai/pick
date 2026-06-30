import { useState, useCallback } from "react";

export type SettingsSectionId = "appearance" | "providers" | "server" | "notifications";

interface SettingsState {
  open: boolean;
  activeSection: SettingsSectionId;
}

let state: SettingsState = { open: false, activeSection: "appearance" };
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((l) => l());
}

export function openSettings(section?: SettingsSectionId) {
  state = { open: true, activeSection: section ?? "appearance" };
  emit();
}

export function closeSettings() {
  state = { open: false, activeSection: "appearance" };
  emit();
}

export function setActiveSettingsSection(section: SettingsSectionId) {
  state = { ...state, activeSection: section };
  emit();
}

export function subscribeToSettings(callback: () => void) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

export function getSettingsSnapshot(): SettingsState {
  return state;
}
