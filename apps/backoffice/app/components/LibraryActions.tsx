"use client";

import { useState, useRef, useEffect, useTransition } from "react";
import Link from "next/link";
import { Button, Badge } from "../components/ui";

interface LibraryActionsProps {
  libraryId: string;
  monitorEnabled: boolean;
  scanMode: string;
  watcherEnabled: boolean;
  onUpdate?: () => void;
}

export function LibraryActions({ 
  libraryId, 
  monitorEnabled, 
  scanMode, 
  watcherEnabled,
  onUpdate 
}: LibraryActionsProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [isPending, startTransition] = useTransition();
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const handleSubmit = (formData: FormData) => {
    startTransition(async () => {
      const monitorEnabled = formData.get("monitor_enabled") === "true";
      const watcherEnabled = formData.get("watcher_enabled") === "true";
      const scanMode = formData.get("scan_mode") as string;
      
      try {
        const response = await fetch(`/api/libraries/${libraryId}/monitoring`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            monitor_enabled: monitorEnabled,
            scan_mode: scanMode,
            watcher_enabled: watcherEnabled,
          }),
        });
        
        if (response.ok) {
          setIsOpen(false);
          window.location.reload();
        } else {
          console.error("Failed to save settings:", response.statusText);
          alert("Failed to save settings. Please try again.");
        }
      } catch (error) {
        console.error("Failed to save settings:", error);
        alert("Failed to save settings. Please try again.");
      }
    });
  };

  return (
    <div className="relative" ref={dropdownRef}>
      <Button 
        variant="ghost" 
        size="sm"
        onClick={() => setIsOpen(!isOpen)}
        className={isOpen ? "bg-muted/10" : ""}
      >
        ⚙️
      </Button>

      {isOpen && (
        <div className="absolute right-0 top-full mt-2 w-72 bg-card rounded-xl shadow-card border border-line p-4 z-50">
          <form action={handleSubmit}>
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <label className="text-sm font-medium text-foreground flex items-center gap-2">
                  <input 
                    type="checkbox" 
                    name="monitor_enabled" 
                    value="true"
                    defaultChecked={monitorEnabled}
                    className="w-4 h-4 rounded border-line text-primary focus:ring-primary"
                  />
                  Auto Scan
                </label>
              </div>

              <div className="flex items-center justify-between">
                <label className="text-sm font-medium text-foreground flex items-center gap-2">
                  <input 
                    type="checkbox" 
                    name="watcher_enabled" 
                    value="true"
                    defaultChecked={watcherEnabled}
                    className="w-4 h-4 rounded border-line text-primary focus:ring-primary"
                  />
                  File Watcher ⚡
                </label>
              </div>

              <div className="flex items-center justify-between">
                <label className="text-sm font-medium text-foreground">📅 Schedule</label>
                <select 
                  name="scan_mode" 
                  defaultValue={scanMode}
                  className="text-sm border border-line rounded-lg px-2 py-1 bg-background"
                >
                  <option value="manual">Manual</option>
                  <option value="hourly">Hourly</option>
                  <option value="daily">Daily</option>
                  <option value="weekly">Weekly</option>
                </select>
              </div>

              <Button 
                type="submit" 
                size="sm" 
                className="w-full"
                disabled={isPending}
              >
                {isPending ? "Saving..." : "Save Settings"}
              </Button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
}
