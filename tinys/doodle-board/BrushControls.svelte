<script lang="ts">
  import { RadioGroup, Slider } from "bits-ui";
  import { defaultBrush, strokePath, type Brush, type Point } from "./drawing";
  let { brush, color, onchange }: { brush: Brush; color: string; onchange: (brush: Brush) => void } = $props();
  const controls = [
    { key: "size", label: "Size", min: 1, max: 64, factor: 1, suffix: "" },
    { key: "smoothing", label: "Smoothing", min: 0, max: 100, factor: 100, suffix: "%" },
    { key: "thinning", label: "Pressure variation", min: 0, max: 100, factor: 100, suffix: "%" },
  ] as const;
  const sample: Point[] = Array.from({ length: 70 }, (_, i) => [30 + i * 3.5, 49 - Math.sin(i / 10) * 18, 0.2 + (Math.sin(i / 13) + 1) * 0.35]);
  const geometry = $derived(strokePath(sample, brush, true));
</script>

<div class="doodle-brush-heading"><h2>Make your mark</h2><button onclick={() => onchange({ ...defaultBrush })}>Reset brush</button></div>
<svg class="doodle-brush-sample" viewBox="0 0 305 100" role="img" aria-label="Brush preview"><path d={geometry} fill={color} /></svg>
{#each controls as control}
  <div class="doodle-brush-control">
    <div class="doodle-brush-label"><span>{control.label}</span><output>{Math.round(brush[control.key] * control.factor)}{control.suffix}</output></div>
    <Slider.Root type="single" min={control.min} max={control.max} step={1} value={Math.round(brush[control.key] * control.factor)} onValueChange={(value) => onchange({ ...brush, [control.key]: value / control.factor })} class="doodle-slider">
      <span class="doodle-slider-track"></span><Slider.Range class="doodle-slider-range" /><Slider.Thumb index={0} class="doodle-slider-thumb" aria-label={control.label} />
    </Slider.Root>
  </div>
{/each}
<div class="doodle-brush-label"><span>Taper</span></div>
<RadioGroup.Root value={brush.taper} onValueChange={(value) => onchange({ ...brush, taper: value as Brush["taper"] })} class="doodle-tapers" aria-label="Taper">
  {#each ["none", "short", "long"] as taper}<RadioGroup.Item value={taper}>{taper[0]!.toUpperCase() + taper.slice(1)}</RadioGroup.Item>{/each}
</RadioGroup.Root>
<p>For your next stroke. Your drawing stays as it is.</p>
