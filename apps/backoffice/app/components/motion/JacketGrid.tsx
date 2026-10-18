"use client";

import { useEffect, useRef, type ReactNode } from "react";

/**
 * Détection de la ligne suivante, alignée sur `<MotionReveal/>` : une large
 * réserve vers le haut (après une restauration de position, le contenu situé
 * au-dessus de l'écran est déjà « vu ») et une petite réserve sous le pli, pour
 * que la ligne suivante attende le défilement.
 */
const REVEAL_MARGIN = "4000px 0px -8% 0px";

/**
 * Rang de la carte dans sa ligne, lu sur la mise en page plutôt que compté :
 * la grille est responsive, donc la largeur — et le nombre de colonnes — change
 * avec la fenêtre. Les voisines qui partagent le même `offsetTop` forment la
 * ligne.
 */
function columnIndexWithinRow(card: HTMLElement): number {
  const parent = card.parentElement;
  if (!parent) return 0;
  const row = card.offsetTop;
  const peers = Array.from(parent.children).filter(
    (sibling): sibling is HTMLElement =>
      sibling instanceof HTMLElement && sibling.offsetTop === row
  );
  return Math.max(peers.indexOf(card), 0);
}

interface JacketGridProps {
  children: ReactNode;
  className?: string;
  /** Cadence d'une carte à l'autre dans une même ligne, en millisecondes. */
  stagger?: number;
}

/**
 * Grille de couvertures révélée ligne par ligne.
 *
 * Chaque carte guette sa propre entrée dans la fenêtre et reçoit un délai égal
 * à son rang dans sa ligne : la ligne se déploie de gauche à droite, et la
 * suivante attend le défilement au lieu de jouer son animation hors écran.
 * L'état caché et le geste d'entrée vivent en CSS (`[data-jackets]` dans
 * `globals.css`), sous le seul drapeau `.js-motion` : sans JavaScript, les
 * couvertures sont simplement là.
 *
 * L'état est écrit dans un attribut `data-jacket` et non dans une classe :
 * React réécrit `className` en bloc quand une carte se re-rend (sélection,
 * badge, compteur de lecture) et effacerait une classe ajoutée à la main.
 */
export function JacketGrid({ children, className, stagger = 90 }: JacketGridProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const container = ref.current;
    if (!container) return;

    const reveal = (card: HTMLElement) => {
      card.style.setProperty("--jacket-delay", `${columnIndexWithinRow(card) * stagger}ms`);
      card.dataset.jacket = "revealed";
    };

    /** Cartes déjà guettées : `observe()` répété est inutile. */
    const watched = new Set<Element>();

    // Pas d'IntersectionObserver (vieux navigateur, jsdom) : on montre tout.
    const observer =
      typeof IntersectionObserver === "undefined"
        ? null
        : new IntersectionObserver(
            (entries) => {
              for (const entry of entries) {
                if (!entry.isIntersecting) continue;
                reveal(entry.target as HTMLElement);
                observer?.unobserve(entry.target);
              }
            },
            { rootMargin: REVEAL_MARGIN, threshold: 0 }
          );

    const watch = () => {
      for (const child of Array.from(container.children)) {
        if (!(child instanceof HTMLElement) || watched.has(child)) continue;
        watched.add(child);
        // Une carte déjà posée garde son état : seules les nouvelles entrent.
        if (child.dataset.jacket === "revealed") continue;
        if (observer) observer.observe(child);
        else reveal(child);
      }
    };
    watch();

    // Pagination, filtres, « tous les tomes » : les cartes montées après coup
    // doivent être guettées à leur tour.
    let frame = 0;
    const mutations =
      typeof MutationObserver === "undefined"
        ? null
        : new MutationObserver(() => {
            if (frame) return;
            frame = window.requestAnimationFrame(() => {
              frame = 0;
              watch();
            });
          });
    mutations?.observe(container, { childList: true });

    return () => {
      observer?.disconnect();
      mutations?.disconnect();
      if (frame) window.cancelAnimationFrame(frame);
    };
  }, [stagger]);

  return (
    <div ref={ref} data-jackets className={className}>
      {children}
    </div>
  );
}
