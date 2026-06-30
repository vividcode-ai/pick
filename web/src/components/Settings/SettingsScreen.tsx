import { useSyncExternalStore } from "react";
import { X, Palette, Cpu, Server, Bell } from "lucide-react";
import {
  closeSettings,
  setActiveSettingsSection,
  subscribeToSettings,
  getSettingsSnapshot,
  type SettingsSectionId,
} from "../../stores/settings";
import { AppearanceSection } from "./AppearanceSection";
import { ProvidersSection } from "./ProvidersSection";
import { ServerSection } from "./ServerSection";
import { NotificationsSection } from "./NotificationsSection";
import type { ProviderInfo } from "../../types/events";

interface SettingsScreenProps {
  providers: ProviderInfo[];
  selectedModel: string;
  onModelChange: (m: string) => void;
  serverUrl: string;
  onSaveServerUrl: (url: string) => void;
}

const navItems: { id: SettingsSectionId; icon: typeof Palette; label: string }[] = [
  { id: "appearance", icon: Palette, label: "Appearance" },
  { id: "providers", icon: Cpu, label: "Providers" },
  { id: "server", icon: Server, label: "Server" },
  { id: "notifications", icon: Bell, label: "Notifications" },
];

export function SettingsScreen({
  providers,
  selectedModel,
  onModelChange,
  serverUrl,
  onSaveServerUrl,
}: SettingsScreenProps) {
  const state = useSyncExternalStore(subscribeToSettings, getSettingsSnapshot, getSettingsSnapshot);

  if (!state.open) return null;

  const renderSection = () => {
    switch (state.activeSection) {
      case "appearance":
        return <AppearanceSection />;
      case "providers":
        return <ProvidersSection providers={providers} selectedModel={selectedModel} onModelChange={onModelChange} />;
      case "server":
        return <ServerSection currentUrl={serverUrl} onSave={onSaveServerUrl} />;
      case "notifications":
        return <NotificationsSection />;
    }
  };

  return (
    <div className="settings-overlay" onClick={closeSettings}>
      <div className="settings-shell" onClick={(e) => e.stopPropagation()}>
        <nav className="settings-nav">
          <div className="settings-nav-header">
            <h2 className="settings-nav-title">Settings</h2>
          </div>
          <div className="settings-nav-list">
            {navItems.map((item) => {
              const Icon = item.icon;
              return (
                <button
                  key={item.id}
                  className="settings-nav-button"
                  data-selected={state.activeSection === item.id}
                  onClick={() => setActiveSettingsSection(item.id)}
                >
                  <Icon className="w-4 h-4" />
                  <span>{item.label}</span>
                </button>
              );
            })}
          </div>
          <div className="flex-1" />
          <div className="text-[10px] text-neutral-500 pt-3 border-t border-neutral-800">
            Pick v0.1.0
          </div>
        </nav>

        <div className="settings-content">
          <header className="settings-content-header">
            <h1 className="settings-content-title">
              {navItems.find((i) => i.id === state.activeSection)?.label}
            </h1>
            <button
              onClick={closeSettings}
              className="p-1.5 rounded-md hover:bg-neutral-800 text-neutral-400 hover:text-neutral-200 transition-colors"
              aria-label="Close settings"
            >
              <X className="w-4 h-4" />
            </button>
          </header>
          <div className="settings-scroll">
            {renderSection()}
          </div>
        </div>
      </div>
    </div>
  );
}
