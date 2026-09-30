<script lang="ts">
import type { StickerKind } from "./schema";

let { kind }: { kind: StickerKind } = $props();
</script>

<svg class="pod-sticker-art" viewBox="0 0 64 64" aria-hidden="true">
  {#if kind === "star"}
    <polygon points="32,4 39.05,22.29 58.63,23.35 43.41,35.71 48.46,54.65 32,44 15.54,54.65 20.59,35.71 5.37,23.35 24.95,22.29" fill="#ffd23f" stroke="#e0a100" stroke-width="2.5" stroke-linejoin="round" />
    <circle cx="26" cy="26" r="2.6" fill="#fff6c9" />
  {:else if kind === "heart"}
    <path d="M32 57 C9 41 5 26 11 18 C18 9 28 11 32 21 C36 11 46 9 53 18 C59 26 55 41 32 57Z" fill="#ff4f7b" stroke="#c91f4f" stroke-width="2.5" stroke-linejoin="round" />
    <path d="M17 22 C19 18 24 17 27 20" fill="none" stroke="#ffc2d2" stroke-width="3" stroke-linecap="round" />
  {:else if kind === "smiley"}
    <circle cx="32" cy="32" r="27" fill="#ffd23f" stroke="#e0a100" stroke-width="2.5" />
    <ellipse cx="23" cy="26" rx="3.6" ry="5.2" fill="#2b2113" />
    <ellipse cx="41" cy="26" rx="3.6" ry="5.2" fill="#2b2113" />
    <path d="M17 38 Q32 54 47 38" fill="none" stroke="#2b2113" stroke-width="4" stroke-linecap="round" />
  {:else if kind === "bolt"}
    <polygon points="37,3 12,36 28,36 22,61 52,24 35,24" fill="#ffe14d" stroke="#d89a00" stroke-width="2.5" stroke-linejoin="round" />
  {:else if kind === "flame"}
    <path d="M32 3 C34 18 51 24 51 41 C51 53 43 61 32 61 C21 61 13 53 13 41 C13 33 18 29 21 22 C24 28 27 30 29 30 C28 20 28 12 32 3Z" fill="#ff7a2f" stroke="#d94e0c" stroke-width="2.5" stroke-linejoin="round" />
    <path d="M32 61 C25 61 21 55 23 48 C25 42 30 40 32 33 C34 40 41 42 41 49 C41 55 38 61 32 61Z" fill="#ffd23f" />
  {:else if kind === "peace"}
    <circle cx="32" cy="32" r="27" fill="#8fe39a" stroke="#2f9a4a" stroke-width="2.5" />
    <path d="M32 9 V55 M32 32 L16 48 M32 32 L48 48" fill="none" stroke="#14532d" stroke-width="4" stroke-linecap="round" />
    <circle cx="32" cy="32" r="18" fill="none" stroke="#14532d" stroke-width="4" />
  {:else if kind === "skull"}
    <path d="M32 5 C16 5 8 17 8 30 C8 38 12 42 16 44 V55 H26 V49 H30 V55 H34 V49 H38 V55 H48 V44 C52 42 56 38 56 30 C56 17 48 5 32 5Z" fill="#f4f4ee" stroke="#2a2a2a" stroke-width="2.5" stroke-linejoin="round" />
    <circle cx="22" cy="30" r="5.5" fill="#2a2a2a" />
    <circle cx="42" cy="30" r="5.5" fill="#2a2a2a" />
    <path d="M32 36 L28 43 H36Z" fill="#2a2a2a" />
  {:else if kind === "cat"}
    <polygon points="9,26 13,4 29,16" fill="#f4a259" stroke="#c9742a" stroke-width="2.5" stroke-linejoin="round" />
    <polygon points="55,26 51,4 35,16" fill="#f4a259" stroke="#c9742a" stroke-width="2.5" stroke-linejoin="round" />
    <ellipse cx="32" cy="37" rx="25" ry="21" fill="#f4a259" stroke="#c9742a" stroke-width="2.5" />
    <ellipse cx="22" cy="34" rx="3" ry="4.4" fill="#2b2113" />
    <ellipse cx="42" cy="34" rx="3" ry="4.4" fill="#2b2113" />
    <path d="M28.5 41 H35.5 L32 45Z" fill="#ff7a95" />
    <path d="M32 45 Q28 50 24 47 M32 45 Q36 50 40 47" fill="none" stroke="#2b2113" stroke-width="2" stroke-linecap="round" />
    <path d="M6 40 H18 M7 46 L18 43 M58 40 H46 M57 46 L46 43" fill="none" stroke="#fff3df" stroke-width="2" stroke-linecap="round" />
  {:else if kind === "note"}
    <circle cx="32" cy="32" r="28" fill="#ffe9a8" stroke="#e0b030" stroke-width="2.5" />
    <polygon points="26,15 49,10 49,20 26,25" fill="#6c5ce7" />
    <path d="M26 20 V44 M49 15 V39" fill="none" stroke="#6c5ce7" stroke-width="4.5" />
    <ellipse cx="19.5" cy="45.5" rx="8.5" ry="6.5" transform="rotate(-20 19.5 45.5)" fill="#6c5ce7" />
    <ellipse cx="42.5" cy="40.5" rx="8.5" ry="6.5" transform="rotate(-20 42.5 40.5)" fill="#6c5ce7" />
  {:else if kind === "rainbow"}
    <path d="M5 46 A27 27 0 0 1 59 46" fill="none" stroke="#ff4d4d" stroke-width="8" />
    <path d="M12.5 46 A19.5 19.5 0 0 1 51.5 46" fill="none" stroke="#ffd23f" stroke-width="8" />
    <path d="M20 46 A12 12 0 0 1 44 46" fill="none" stroke="#4dabf7" stroke-width="8" />
    <circle cx="8" cy="48" r="7" fill="#fff" stroke="#c8d4e0" stroke-width="2" />
    <circle cx="56" cy="48" r="7" fill="#fff" stroke="#c8d4e0" stroke-width="2" />
  {:else if kind === "pizza"}
    <path d="M32 60 L5 15 Q32 1 59 15Z" fill="#ffcf4a" stroke="#d98b00" stroke-width="2.5" stroke-linejoin="round" />
    <path d="M5 15 Q32 1 59 15 L56 21 Q32 8 8 21Z" fill="#e98a3a" stroke="#c46a1c" stroke-width="2" stroke-linejoin="round" />
    <circle cx="24" cy="28" r="5" fill="#d6322f" />
    <circle cx="40" cy="28" r="5" fill="#d6322f" />
    <circle cx="32" cy="43" r="5" fill="#d6322f" />
  {:else if kind === "alien"}
    <path d="M32 4 C14 4 7 20 11 36 C15 50 24 60 32 60 C40 60 49 50 53 36 C57 20 50 4 32 4Z" fill="#7bd88f" stroke="#2f9a4a" stroke-width="2.5" stroke-linejoin="round" />
    <ellipse cx="22" cy="31" rx="9" ry="5.2" transform="rotate(25 22 31)" fill="#122014" />
    <ellipse cx="42" cy="31" rx="9" ry="5.2" transform="rotate(-25 42 31)" fill="#122014" />
    <path d="M27 47 Q32 50 37 47" fill="none" stroke="#122014" stroke-width="2.5" stroke-linecap="round" />
  {:else if kind === "cassette"}
    <rect x="4" y="12" width="56" height="40" rx="5" fill="#3b3f46" stroke="#1d2024" stroke-width="2.5" />
    <rect x="10" y="17" width="44" height="17" rx="2" fill="#f5e6c4" />
    <path d="M14 24 H50 M14 29 H38" fill="none" stroke="#c9a85a" stroke-width="2" stroke-linecap="round" />
    <rect x="19" y="38" width="26" height="9" rx="4.5" fill="#1c1e22" />
    <circle cx="26" cy="42.5" r="3.2" fill="#c9ccd1" />
    <circle cx="38" cy="42.5" r="3.2" fill="#c9ccd1" />
  {:else if kind === "hello"}
    <rect x="3" y="10" width="58" height="44" rx="6" fill="#ff5a5f" stroke="#c4343a" stroke-width="2.5" />
    <rect x="7" y="27" width="50" height="23" rx="3" fill="#fff" />
    <text x="32" y="23" text-anchor="middle" font-size="12" font-weight="800" fill="#fff" font-family="Helvetica Neue, Helvetica, Arial, sans-serif">HELLO</text>
    <text x="32" y="36" text-anchor="middle" font-size="6" fill="#444" font-family="Helvetica Neue, Helvetica, Arial, sans-serif">my name is</text>
    <path d="M14 45 Q20 40 26 45 T38 45 T50 44" fill="none" stroke="#ff5a5f" stroke-width="2" stroke-linecap="round" />
  {:else if kind === "cloud"}
    <g fill="#bfe3ff" stroke="#6fb4ee" stroke-width="2.5">
      <circle cx="20" cy="38" r="12" />
      <circle cx="33" cy="29" r="15" />
      <circle cx="47" cy="38" r="11" />
      <rect x="20" y="38" width="27" height="12" />
    </g>
    <rect x="21" y="35" width="25" height="13" fill="#bfe3ff" />
    <ellipse cx="27" cy="36" rx="2.2" ry="3" fill="#24425f" />
    <ellipse cx="40" cy="36" rx="2.2" ry="3" fill="#24425f" />
    <path d="M29 42 Q33.5 46 38 42" fill="none" stroke="#24425f" stroke-width="2" stroke-linecap="round" />
    <circle cx="23" cy="42" r="2.4" fill="#ff9db8" />
    <circle cx="44" cy="42" r="2.4" fill="#ff9db8" />
  {:else if kind === "fan"}
    <circle cx="32" cy="32" r="28" fill="#ff4d8d" stroke="#c21f61" stroke-width="2.5" />
    <circle cx="32" cy="32" r="22.5" fill="none" stroke="#fff" stroke-width="1.8" stroke-dasharray="3 3" />
    <text x="32" y="37" text-anchor="middle" font-size="22" font-weight="900" fill="#fff" font-family="Helvetica Neue, Helvetica, Arial, sans-serif">#1</text>
    <text x="32" y="48" text-anchor="middle" font-size="9" font-weight="800" fill="#fff" font-family="Helvetica Neue, Helvetica, Arial, sans-serif">FAN</text>
  {/if}
</svg>
