import { useId } from "react";

interface DockFolderIconProps {
  className?: string;
}

/**
 * Folder-with-fanned-documents vector icon styled for the dock tab, peek header, and panel header.
 */
export function DockFolderIcon({ className }: DockFolderIconProps) {
  const uid = useId().replace(/:/g, "");
  const folderGradId = `folder-grad-${uid}`;
  const sheetGradId = `sheet-grad-${uid}`;
  const checkGradId = `check-grad-${uid}`;

  return (
    <svg
      viewBox="0 0 64 64"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      aria-hidden="true"
    >
      <defs>
        <linearGradient id={folderGradId} x1="32" y1="26" x2="32" y2="60" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="#FFC85E" />
          <stop offset="100%" stopColor="#F5B041" />
        </linearGradient>
        <linearGradient id={sheetGradId} x1="39" y1="10" x2="39" y2="42" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="#F1F8FF" />
          <stop offset="100%" stopColor="#E1EEFB" />
        </linearGradient>
        <linearGradient id={checkGradId} x1="52" y1="38" x2="52" y2="58" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="#22C55E" />
          <stop offset="100%" stopColor="#16A34A" />
        </linearGradient>
      </defs>

      {/* Dark teal rear document fanned to upper-left */}
      <g transform="rotate(-44 27 32)">
        <rect x="6" y="12" width="24" height="32" rx="2" fill="#005B82" />
      </g>

      {/* Sky-blue middle document fanned slightly left */}
      <g transform="rotate(-21 31 32)">
        <rect
          x="15"
          y="9"
          width="27"
          height="34"
          rx="2.2"
          fill="#55B7FF"
          stroke="#1082D4"
          strokeWidth="1.2"
        />
      </g>

      {/* Front ice-white folded-corner markdown page */}
      <path
        d="M27.5 11H46.2L53.5 18.3V42H26V12.5C26 11.67 26.67 11 27.5 11Z"
        fill={`url(#${sheetGradId})`}
      />
      {/* Subtle right-edge depth strip on front sheet */}
      <path d="M51 18.3H53.5V42H51V18.3Z" fill="#D2DFEC" />
      {/* Top-right folded dog-ear flap */}
      <path
        d="M46.2 11V16.8C46.2 17.63 46.87 18.3 47.7 18.3H53.5L46.2 11Z"
        fill="#BCC7D4"
      />

      {/* Document content lines */}
      <path
        d="M30.5 19H43M30.5 24H48.5M30.5 29H48.5M30.5 34H48.5"
        stroke="#B5C0CC"
        strokeWidth="2.4"
        strokeLinecap="round"
      />

      {/* Warm golden-amber front folder pocket */}
      <path
        d="M12.5 30.5C12.5 28.01 14.51 26 17 26H27.4C29.02 26 30.53 26.86 31.34 28.27L33.1 31.3C33.82 32.54 35.14 33.3 36.57 33.3H50.5C52.99 33.3 55 35.31 55 37.8V53.5C55 56.81 52.31 59.5 49 59.5H18.5C15.19 59.5 12.5 56.81 12.5 53.5V30.5Z"
        fill={`url(#${folderGradId})`}
      />

      {/* Subtle inner bevel along upper pocket edge & bottom-right edge */}
      <path
        d="M27.4 26C29.02 26 30.53 26.86 31.34 28.27L33.1 31.3C33.82 32.54 35.14 33.3 36.57 33.3H50.5C51.6 33.3 52.61 33.7 53.39 34.36C52.67 34.95 51.73 35.3 50.65 35.3H35.8C34.22 35.3 32.76 34.48 31.95 33.14L30.2 30.2C29.45 28.96 28.1 28.2 26.65 28.2H17C15.2 28.2 13.65 29.15 12.82 30.58C13.07 27.99 15.24 26 17.9 26H27.4Z"
        fill="#E09D3B"
        opacity="0.65"
      />
      <path
        d="M55 39.5V53.5C55 56.81 52.31 59.5 49 59.5H18.5C16.12 59.5 14.07 58.11 13.1 56.1C14.12 57.15 15.54 57.8 17.1 57.8H47.6C50.91 57.8 53.5 55.21 53.5 51.9V37.8C53.5 36.5 53.05 35.31 52.3 34.37C53.91 35.18 55 36.85 55 39.5Z"
        fill="#DA9635"
        opacity="0.7"
      />

      {/* Embossed folder label lines */}
      <path
        d="M18.5 49H28.5M18.5 54H46.5"
        stroke="#B67F2E"
        strokeWidth="2.5"
        strokeLinecap="round"
      />

      {/* Signature emerald green checkmark circle badge */}
      <circle cx="52.5" cy="48.5" r="9.5" fill={`url(#${checkGradId})`} />
      <path
        d="M48.2 48.8L51.1 51.7L57.1 45.6"
        stroke="#FFFFFF"
        strokeWidth="2.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
