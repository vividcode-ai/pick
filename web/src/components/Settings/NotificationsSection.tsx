export function NotificationsSection() {
  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-sm font-semibold text-neutral-100">Notifications</h3>
        <p className="text-xs text-neutral-500 mt-1">Configure notification preferences</p>
      </div>
      <div className="settings-card space-y-3">
        <div className="settings-row">
          <div>
            <div className="settings-row-label">Desktop Notifications</div>
            <div className="settings-row-description">Show notifications when responses are ready</div>
          </div>
          <label className="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" className="sr-only peer" defaultChecked />
            <div className="w-9 h-5 bg-neutral-700 peer-checked:bg-blue-600 rounded-full after:content-[''] after:absolute after:top-0.5 after:start-[2px] after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:after:translate-x-full" />
          </label>
        </div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">Sound</div>
            <div className="settings-row-description">Play a sound when a response completes</div>
          </div>
          <label className="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" className="sr-only peer" />
            <div className="w-9 h-5 bg-neutral-700 peer-checked:bg-blue-600 rounded-full after:content-[''] after:absolute after:top-0.5 after:start-[2px] after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:after:translate-x-full" />
          </label>
        </div>
      </div>
    </div>
  );
}
