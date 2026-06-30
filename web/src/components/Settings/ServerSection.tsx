import { useState } from "react";

interface ServerSectionProps {
  currentUrl: string;
  onSave: (url: string) => void;
}

export function ServerSection({ currentUrl, onSave }: ServerSectionProps) {
  const [url, setUrl] = useState(currentUrl);

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-sm font-semibold text-neutral-100">Server</h3>
        <p className="text-xs text-neutral-500 mt-1">Configure the backend server connection</p>
      </div>
      <div className="settings-card">
        <label className="block text-sm text-neutral-400 mb-1.5">Server URL</label>
        <input
          type="text"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="http://localhost:8080"
          className="w-full px-3 py-2 bg-neutral-800 border border-neutral-700 rounded text-neutral-100 text-sm outline-none focus:border-blue-500 transition-colors"
        />
        <p className="text-xs text-neutral-500 mt-1.5">
          The URL of the Pick backend server. Changes require a reload to take effect.
        </p>
        <button
          onClick={() => onSave(url)}
          className="mt-3 px-4 py-1.5 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors"
        >
          Save & Reload
        </button>
      </div>
    </div>
  );
}
