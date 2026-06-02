"use client";

import { useState, useTransition } from "react";
import { Modal } from "./ui/Modal";
import { Button } from "./ui";

export function UserGenreRestrictions({
  userId,
  initialBlockedGenres,
  allGenres,
  action,
}: {
  userId: string;
  initialBlockedGenres: string[];
  allGenres: string[];
  action: (formData: FormData) => Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set(initialBlockedGenres));
  const [pending, startTransition] = useTransition();

  function toggle(genre: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(genre)) next.delete(genre);
      else next.add(genre);
      return next;
    });
  }

  function save() {
    const fd = new FormData();
    fd.append("id", userId);
    fd.append("blocked_genres", JSON.stringify(Array.from(selected).sort()));
    startTransition(async () => {
      await action(fd);
      setOpen(false);
    });
  }

  function openModal() {
    setSelected(new Set(initialBlockedGenres));
    setOpen(true);
  }

  const blockedCount = initialBlockedGenres.length;

  return (
    <>
      <Button type="button" variant="outline" size="xs" onClick={openModal}>
        <svg className="w-3.5 h-3.5 mr-1.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" />
        </svg>
        Gérer les restrictions
      </Button>

      <Modal isOpen={open} onClose={() => !pending && setOpen(false)} title="Restrictions" maxWidth="sm">
        <div className="p-5 space-y-4">
          <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Genres</p>
          {allGenres.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-4">Aucun genre disponible</p>
          ) : (
            <div className="space-y-1 max-h-72 overflow-y-auto pr-1">
              {allGenres.map((genre) => (
                <label
                  key={genre}
                  className="flex items-center gap-2.5 px-2 py-1.5 rounded hover:bg-accent/50 cursor-pointer group"
                >
                  <input
                    type="checkbox"
                    checked={selected.has(genre)}
                    onChange={() => toggle(genre)}
                    className="accent-destructive w-4 h-4 shrink-0"
                  />
                  <span className={`text-sm ${selected.has(genre) ? "text-destructive font-medium line-through opacity-70" : "text-foreground"}`}>
                    {genre}
                  </span>
                </label>
              ))}
            </div>
          )}

          <div className="flex items-center justify-between pt-2 border-t border-border/50">
            <span className="text-xs text-muted-foreground">
              {selected.size > 0 ? `${selected.size} genre${selected.size > 1 ? "s" : ""} bloqué${selected.size > 1 ? "s" : ""}` : "Aucune restriction"}
            </span>
            <div className="flex gap-2">
              <Button variant="outline" size="xs" onClick={() => setOpen(false)} disabled={pending}>
                Annuler
              </Button>
              <Button variant="default" size="xs" onClick={save} disabled={pending}>
                {pending ? "Enregistrement…" : "Enregistrer"}
              </Button>
            </div>
          </div>
        </div>
      </Modal>
    </>
  );
}
