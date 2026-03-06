"use client";

import { useTransition } from "react";

interface MonitoringFormProps {
  libraryId: string;
  monitorEnabled: boolean;
  scanMode: string;
  watcherEnabled: boolean;
}

export function MonitoringForm({ libraryId, monitorEnabled, scanMode, watcherEnabled }: MonitoringFormProps) {
  const [isPending, startTransition] = useTransition();

  const handleSubmit = (formData: FormData) => {
    startTransition(async () => {
      try {
        const response = await fetch(`/api/libraries/${libraryId}/monitoring`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            monitor_enabled: formData.get("monitor_enabled") === "true",
            scan_mode: formData.get("scan_mode"),
            watcher_enabled: formData.get("watcher_enabled") === "true",
          }),
        });
        if (response.ok) {
          window.location.reload();
        }
      } catch (error) {
        console.error("Failed to update monitoring:", error);
      }
    });
  };

  return (
    <form action={handleSubmit} className="flex items-center gap-2">
      <input type="hidden" name="id" value={libraryId} />
      
      <div className="flex items-center gap-2">
        <label className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border text-sm font-medium transition-all cursor-pointer select-none ${
          isPending 
            ? 'opacity-50 cursor-not-allowed' 
            : 'hover:border-primary'
        } ${monitorEnabled ? 'bg-primary-soft border-primary text-primary' : 'bg-card border-line text-muted'}`}>
          <input
            type="checkbox"
            name="monitor_enabled"
            value="true"
            defaultChecked={monitorEnabled}
            disabled={isPending}
            className="w-3.5 h-3.5 rounded border-line text-primary focus:ring-primary"
          />
          <span>Auto</span>
        </label>
        
        <label className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border text-sm font-medium transition-all cursor-pointer select-none ${
          isPending 
            ? 'opacity-50 cursor-not-allowed' 
            : 'hover:border-primary'
        } ${watcherEnabled ? 'bg-warning-soft border-warning text-warning' : 'bg-card border-line text-muted'}`}>
          <input
            type="checkbox"
            name="watcher_enabled"
            value="true"
            defaultChecked={watcherEnabled}
            disabled={isPending}
            className="w-3.5 h-3.5 rounded border-line text-warning focus:ring-warning"
          />
          <span title="Real-time file watcher">⚡</span>
        </label>
        
        <select
          name="scan_mode"
          defaultValue={scanMode}
          disabled={isPending}
          className="px-3 py-1.5 text-sm rounded-lg border border-line bg-card text-foreground focus:ring-2 focus:ring-primary focus:border-primary disabled:opacity-50"
        >
          <option value="manual">Manual</option>
          <option value="hourly">Hourly</option>
          <option value="daily">Daily</option>
          <option value="weekly">Weekly</option>
        </select>
        
        <button 
          type="submit" 
          disabled={isPending}
          className="flex items-center justify-center w-8 h-8 rounded-lg bg-primary text-white font-semibold text-sm transition-all hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isPending ? '...' : '✓'}
        </button>
      </div>
    </form>
  );
}
