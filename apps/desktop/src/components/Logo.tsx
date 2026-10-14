/** The Lumina mark; same artwork as assets/branding/app-icon.svg. */
export function Logo({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 1024 1024" className={className} aria-hidden>
      <defs>
        <linearGradient id="lumina-bg" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#6366F1" />
          <stop offset="1" stopColor="#7C3AED" />
        </linearGradient>
        <radialGradient id="lumina-glow" cx="0.4" cy="0.4" r="0.6">
          <stop offset="0" stopColor="#FEF3C7" />
          <stop offset="1" stopColor="#F59E0B" />
        </radialGradient>
      </defs>
      <rect x="64" y="64" width="896" height="896" rx="208" fill="url(#lumina-bg)" />
      <path
        d="M380 280 V700 H680"
        fill="none"
        stroke="#fff"
        strokeWidth="112"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="668" cy="336" r="92" fill="url(#lumina-glow)" />
    </svg>
  );
}
