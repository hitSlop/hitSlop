<script lang="ts">
  // A corner of the sheet on felt: five staff lines, a phrase with the cursor on it,
  // and the brass tempo stamp showing the current tempo.
  import doc from "./schema";
  const tempo = $derived(doc.current.tempo);
  const notes = [
    { x: 118, y: 196 }, { x: 170, y: 226 }, { x: 214, y: 214 }, { x: 258, y: 202 }, { x: 302, y: 190 },
  ];
</script>

<div class="ps-icon" aria-hidden="true">
  <svg viewBox="0 0 456 456" width="456" height="456">
    <rect class="ps-icon-felt" x="0" y="0" width="456" height="456" rx="96" />
    <g transform="rotate(-4 228 228)">
      <rect class="ps-icon-paper" x="62" y="88" width="332" height="290" rx="18" />
      {#each [0, 1, 2, 3, 4] as line}
        <line class="ps-icon-staff" x1="92" x2="364" y1={178 + line * 24} y2={178 + line * 24} />
      {/each}
      <rect class="ps-icon-cursor" x="207" y="160" width="10" height="132" rx="5" />
      {#each notes as note, i}
        <ellipse class={i === 2 ? "ps-icon-note ps-icon-lit" : "ps-icon-note"} cx={note.x} cy={note.y + 24} rx="17" ry="12.5" transform={`rotate(-20 ${note.x} ${note.y + 24})`} />
        <line class={i === 2 ? "ps-icon-stem ps-icon-lit-stem" : "ps-icon-stem"} x1={note.x + 15} x2={note.x + 15} y1={note.y + 20} y2={note.y - 58} />
      {/each}
      <path class="ps-icon-beam" d="M 271 144 L 319 132 L 319 145 L 271 157 Z" />
    </g>
    <circle class="ps-icon-stamp" cx="338" cy="352" r="72" />
    <text class="ps-icon-tempo" x="338" y="372" text-anchor="middle">{tempo}</text>
  </svg>
</div>
