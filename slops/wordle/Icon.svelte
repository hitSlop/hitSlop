<script lang="ts">
import "hitslop/base.css";
import "./styles.css";
import type { CaptureProps } from "hitslop/svelte";
import type schema from "./schema";
let { snapshot }: CaptureProps<typeof schema> = $props();
const activeState = $derived(snapshot.mode === "daily" ? snapshot.daily : snapshot.practice);
const badge = $derived(activeState.status === "won" ? `${activeState.guesses.length}/6` : activeState.status === "lost" ? "X/6" : "SLOPS");
</script>

<div class="iconSurface" aria-hidden="true">
  <div class="iconBezel">
    <div class="iconBrandStrip">
      <span class="iconBrandDot"></span>
      <span class="iconBrandTitle">WORDLE</span>
      <span class="iconBrandDot"></span>
    </div>
    <div class="iconGrid">
      <div class="iconRow">
        <div class="iconTile" data-tone="absent">W</div>
        <div class="iconTile" data-tone="present">O</div>
        <div class="iconTile" data-tone="absent">R</div>
        <div class="iconTile" data-tone="absent">D</div>
        <div class="iconTile" data-tone="present">S</div>
      </div>
      <div class="iconRow">
        <div class="iconTile" data-tone="correct">S</div>
        <div class="iconTile" data-tone="correct">L</div>
        <div class="iconTile" data-tone="correct">O</div>
        <div class="iconTile" data-tone="correct">P</div>
        <div class="iconTile" data-tone="correct">S</div>
      </div>
      <div class="iconRow">
        <div class="iconTile" data-tone="empty"></div>
        <div class="iconTile" data-tone="empty"></div>
        <div class="iconTile" data-tone="empty"></div>
        <div class="iconTile" data-tone="empty"></div>
        <div class="iconTile" data-tone="empty"></div>
      </div>
    </div>
    <div class="iconIndicator">
      <div class="iconBar"></div>
      <span class="iconBadge">{badge}</span>
    </div>
  </div>
</div>
