"use client";

import { ReactNode, useState, useRef } from "react";
import { useTranslation } from "@/lib/i18n/context";

interface Library {
  id: string;
  name: string;
}

interface LibraryBadgeSelectorProps {
  libraries: Library[];
  children: ReactNode;
  initialSelected?: string | null;
}

export function LibraryBadgeSelector({ libraries, children, initialSelected = null }: LibraryBadgeSelectorProps) {
  const { t } = useTranslation();
  // null = no selection, "" = all, string = specific library id
  const [selected, setSelected] = useState<string | null>(initialSelected);
  const [warning, setWarning] = useState(false);
  const formRef = useRef<HTMLFormElement>(null);

  function handleSelect(value: string) {
    setWarning(false);
    setSelected(selected === value ? null : value);
  }

  function handleSubmit(e: React.FormEvent<HTMLFormElement>) {
    if (selected === null) {
      e.preventDefault();
      setWarning(true);
    }
  }

  const isActive = (value: string) => selected === value;

  return (
    <form ref={formRef} onSubmit={handleSubmit}>
      <input type="hidden" name="library_id" value={selected ?? ""} />
      <div className="mb-6">
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => handleSelect("")}
            className={`cursor-pointer inline-flex items-center gap-1.5 rounded-full px-4 py-1.5 text-sm font-medium border-2 transition-all duration-150 ${
              isActive("")
                ? "border-primary bg-primary/10 text-primary dark:bg-primary/20"
                : "border-transparent bg-accent/60 text-muted-foreground hover:bg-accent hover:text-foreground"
            }`}
          >
            {isActive("") && (
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" />
              </svg>
            )}
            {t("jobs.allLibraries")}
          </button>
          {libraries.map((lib) => (
            <button
              key={lib.id}
              type="button"
              onClick={() => handleSelect(lib.id)}
              className={`cursor-pointer inline-flex items-center gap-1.5 rounded-full px-4 py-1.5 text-sm font-medium border-2 transition-all duration-150 ${
                isActive(lib.id)
                  ? "border-primary bg-primary/10 text-primary dark:bg-primary/20"
                  : "border-transparent bg-accent/60 text-muted-foreground hover:bg-accent hover:text-foreground"
              }`}
            >
              {isActive(lib.id) && (
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" />
                </svg>
              )}
              {lib.name}
            </button>
          ))}
        </div>
        {warning && (
          <p className="text-sm text-destructive mt-2">
            {t("jobs.noLibraryWarning")}
          </p>
        )}
      </div>
      {children}
    </form>
  );
}
