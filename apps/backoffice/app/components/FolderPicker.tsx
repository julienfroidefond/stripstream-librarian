"use client";

import { useState } from "react";
import { FolderBrowser } from "./FolderBrowser";
import { FolderItem } from "../../lib/api";
import { Button } from "./ui";

interface FolderPickerProps {
  initialFolders: FolderItem[];
  selectedPath: string;
  onSelect: (path: string) => void;
}

export function FolderPicker({ initialFolders, selectedPath, onSelect }: FolderPickerProps) {
  const [isOpen, setIsOpen] = useState(false);

  const handleSelect = (path: string) => {
    onSelect(path);
    setIsOpen(false);
  };

  return (
    <div className="relative">
      {/* Input avec bouton browse */}
      <div className="flex items-center gap-2">
        <div className="flex-1 relative">
          <input
            type="text"
            readOnly
            value={selectedPath || "Select a folder..."}
            className={`
              w-full px-3 py-2 rounded-lg border bg-card
              text-sm font-mono
              ${selectedPath ? 'text-foreground' : 'text-muted-foreground italic'}
              border-border/50 focus:border-primary/50 focus:ring-2 focus:ring-primary/20
              transition-all duration-200
            `}
          />
          {selectedPath && (
            <button
              type="button"
              onClick={() => onSelect("")}
              className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-destructive transition-colors"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          )}
        </div>
        <Button
          type="button"
          variant="secondary"
          onClick={() => setIsOpen(true)}
          className="flex items-center gap-2"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
          </svg>
          Browse
        </Button>
      </div>

      {/* Popup Modal */}
      {isOpen && (
        <>
          {/* Backdrop */}
          <div 
            className="fixed inset-0 bg-black/30 backdrop-blur-sm z-50"
            onClick={() => setIsOpen(false)}
          />
          
          {/* Modal */}
          <div className="fixed inset-0 flex items-center justify-center z-50 p-4">
            <div className="bg-card border border-border/50 rounded-xl shadow-2xl w-full max-w-lg overflow-hidden animate-in fade-in zoom-in-95 duration-200">
              {/* Header */}
              <div className="flex items-center justify-between px-4 py-3 border-b border-border/50 bg-muted/30">
                <div className="flex items-center gap-2">
                  <svg className="w-5 h-5 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                  </svg>
                  <span className="font-medium">Select Folder</span>
                </div>
                <button
                  type="button"
                  onClick={() => setIsOpen(false)}
                  className="text-muted-foreground hover:text-foreground transition-colors p-1 hover:bg-accent rounded"
                >
                  <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>

              {/* Folder Browser */}
              <div className="p-0">
                <FolderBrowser
                  initialFolders={initialFolders}
                  selectedPath={selectedPath}
                  onSelect={handleSelect}
                />
              </div>

              {/* Footer */}
              <div className="flex items-center justify-between px-4 py-3 border-t border-border/50 bg-muted/30">
                <span className="text-xs text-muted-foreground">
                  Click a folder to select it
                </span>
                <div className="flex gap-2">
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => setIsOpen(false)}
                  >
                    Cancel
                  </Button>
                </div>
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
