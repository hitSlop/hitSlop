<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { trayView } from "./dice";

  let { snapshot }: CaptureProps<typeof schema> = $props();
  // The tray from above: walnut rim, felt well, a d20 showing the last total's die and a d6.
  const view = $derived(trayView(snapshot));
  const face = $derived(view.lastView?.values.find((die) => die.kind === "d20")?.value ?? 20);
</script>

<svg class="tray-icon" viewBox="0 0 400 400" aria-hidden="true">
  <rect x="22" y="22" width="356" height="356" rx="64" fill="var(--slop-wood)" />
  <rect x="22" y="22" width="356" height="356" rx="64" fill="none" stroke="var(--slop-woodDark)" stroke-width="10" />
  <rect x="58" y="58" width="284" height="284" rx="38" fill="var(--slop-felt)" />
  <rect x="58" y="58" width="284" height="284" rx="38" fill="none" stroke="var(--slop-feltDeep)" stroke-width="14" opacity="0.7" />
  <ellipse cx="200" cy="292" rx="96" ry="18" fill="var(--slop-feltDeep)" opacity="0.6" />
  <g transform="translate(92 78) scale(2.1)">
    <polygon points="50,3 92,27 92,73 50,97 8,73 8,27" fill="var(--slop-die)" stroke="var(--slop-pip)" stroke-width="3" stroke-linejoin="round" />
    <path d="M50 26L78 70H22Z M50 3V26 M92 27L78 70 M8 27L22 70 M50 26L8 27 M50 26L92 27 M22 70L50 97L78 70 M92 73L78 70 M8 73L22 70" fill="none" stroke="var(--slop-pip)" stroke-width="1.6" opacity="0.45" />
    <text x="50" y="56" text-anchor="middle" dominant-baseline="middle" font-family="'Dice Tray Serif', Georgia, serif" font-weight="800" font-size="24" fill="var(--slop-pip)">{face}</text>
  </g>
  <g transform="translate(262 236) rotate(14)">
    <rect width="62" height="62" rx="11" fill="var(--slop-die)" stroke="var(--slop-pip)" stroke-width="3" />
    <circle cx="17" cy="17" r="6" fill="var(--slop-pip)" /><circle cx="45" cy="45" r="6" fill="var(--slop-pip)" /><circle cx="31" cy="31" r="6" fill="var(--slop-pip)" />
  </g>
</svg>
