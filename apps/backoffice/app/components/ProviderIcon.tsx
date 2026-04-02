/** Inline SVG icons for metadata providers */

interface ProviderIconProps {
  provider: string;
  size?: number;
  className?: string;
}

export function ProviderIcon({ provider, size = 16, className = "" }: ProviderIconProps) {
  const style = { width: size, height: size, flexShrink: 0 };

  switch (provider) {
    case "google_books":
      // Stylized book (Google Books)
      return (
        <svg viewBox="0 0 24 24" style={style} className={className}>
          <path
            d="M21 4H3a1 1 0 0 0-1 1v14a1 1 0 0 0 1 1h18a1 1 0 0 0 1-1V5a1 1 0 0 0-1-1z"
            fill="#4285F4"
            opacity="0.15"
          />
          <path
            d="M12 4v16M6 4h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z"
            fill="none"
            stroke="#4285F4"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path d="M7 8h3M7 11h3M14 8h3M14 11h3" stroke="#4285F4" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      );

    case "open_library":
      // Open book (Open Library)
      return (
        <svg viewBox="0 0 24 24" style={style} className={className}>
          <path
            d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
            fill="none"
            stroke="#E8590C"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      );

    case "comicvine":
      // Explosion / star burst (ComicVine)
      return (
        <svg viewBox="0 0 24 24" style={style} className={className}>
          <path
            d="M12 2l2.09 6.26L20.18 9l-4.91 4.09L16.54 20 12 16.27 7.46 20l1.27-6.91L3.82 9l6.09-.74z"
            fill="#E7272D"
            opacity="0.15"
          />
          <path
            d="M12 2l2.09 6.26L20.18 9l-4.91 4.09L16.54 20 12 16.27 7.46 20l1.27-6.91L3.82 9l6.09-.74z"
            fill="none"
            stroke="#E7272D"
            strokeWidth="1.5"
            strokeLinejoin="round"
          />
        </svg>
      );

    case "anilist":
      // Stylized play / triangle (AniList)
      return (
        <svg viewBox="0 0 24 24" style={style} className={className}>
          <rect x="3" y="3" width="18" height="18" rx="3" fill="#02A9FF" opacity="0.15" />
          <path
            d="M8 6h2.5l4 12H12l-.75-2.5H7.75L7 18H4.5L8 6zm-.25 7.5h3.5L9.5 8.25 7.75 13.5z"
            fill="#02A9FF"
          />
          <path d="M16 10h2.5v8H16z" fill="#02A9FF" />
        </svg>
      );

    case "bedetheque":
      // French flag-inspired book (Bédéthèque)
      return (
        <svg viewBox="0 0 24 24" style={style} className={className}>
          <rect x="3" y="4" width="6" height="16" rx="1" fill="#002395" opacity="0.2" />
          <rect x="9" y="4" width="6" height="16" fill="#FFFFFF" opacity="0.1" />
          <rect x="15" y="4" width="6" height="16" rx="1" fill="#ED2939" opacity="0.2" />
          <path
            d="M6 4h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z"
            fill="none"
            stroke="#002395"
            strokeWidth="1.5"
            strokeLinejoin="round"
          />
          <path d="M8 9h8M8 12h6M8 15h4" stroke="#002395" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      );

    case "senscritique":
      // SC stylized rating badge
      return (
        <svg viewBox="0 0 24 24" style={style} className={className}>
          <rect x="3" y="3" width="18" height="18" rx="3" fill="#1A1A2E" opacity="0.15" />
          <text x="12" y="16" textAnchor="middle" fontSize="11" fontWeight="bold" fill="#E4003A" fontFamily="sans-serif">SC</text>
        </svg>
      );

    default:
      // Generic globe
      return (
        <svg viewBox="0 0 24 24" style={style} className={className} fill="none" stroke="currentColor" strokeWidth="1.5">
          <circle cx="12" cy="12" r="9" />
          <path d="M3.6 9h16.8M3.6 15h16.8M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0 18" strokeLinecap="round" />
        </svg>
      );
  }
}

export const PROVIDERS = [
  { value: "google_books", label: "Google Books" },
  { value: "open_library", label: "Open Library" },
  { value: "comicvine", label: "ComicVine" },
  { value: "anilist", label: "AniList" },
  { value: "bedetheque", label: "Bédéthèque" },
  { value: "senscritique", label: "SensCritique" },
] as const;

export function providerLabel(value: string) {
  return PROVIDERS.find((p) => p.value === value)?.label ?? value.replace("_", " ");
}
