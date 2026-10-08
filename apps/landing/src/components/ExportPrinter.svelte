<script lang="ts">
  type Format = "pdf" | "png";
  let printing = $state<Format | null>(null);
  let printed = $state<Format | null>(null);
  let run = $state(0);
  const files: Record<Format, { name: string; note: string }> = {
    pdf: { name: "Invoice.pdf", note: "selectable text · vector" },
    png: { name: "Invoice.png", note: "2× crisp · full height" },
  };

  function exportAs(format: Format): void {
    if (printing) return;
    run += 1;
    printed = null;
    printing = format;
    const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
    setTimeout(() => { printed = format; printing = null; }, reduce ? 0 : 1500);
  }
</script>

<div class="export">
  <div class="buttons" role="group" aria-label="Export the invoice">
    <button type="button" class="button button-primary" onclick={() => exportAs("pdf")} disabled={!!printing}>Export PDF</button>
    <button type="button" class="button button-secondary" onclick={() => exportAs("png")} disabled={!!printing}>Export PNG</button>
  </div>

  <div class="stage">
    <div class="printer" aria-hidden="true">
      <div class="printer-top"><span class="led" class:on={!!printing}></span><span class="slot"></span></div>
      <div class="printer-body"><span>hitSlop</span></div>
    </div>
    {#key run}
      <div class="paper" data-state={printing ? "printing" : printed ? "done" : "idle"}>
        <header><span>Invoice</span><b>PAID</b></header>
        <p>Northwind Studio</p>
        <div><span>Product design</span><strong>$2,500</strong></div>
        <div><span>Development</span><strong>$1,440</strong></div>
        <footer><span>Total</span><strong>$3,940</strong></footer>
      </div>
    {/key}
    {#if printed}
      <p class="file sticker" data-tone={printed === "pdf" ? "pink" : "sky"} style="--tilt: 6deg" role="status">{printed === "pdf" ? "📄" : "🖼️"} {files[printed].name}<small>{files[printed].note}</small></p>
    {/if}
  </div>
</div>

<style>
  .export { display: grid; gap: 24px; justify-items: center; }
  .buttons { display: flex; flex-wrap: wrap; justify-content: center; gap: 10px; }
  .buttons .button:disabled { opacity: .6; cursor: progress; }
  .stage { position: relative; width: min(100%, 420px); min-height: 470px; }
  .printer { position: relative; z-index: 2; width: 100%; }
  .printer-top { height: 42px; margin: 0 26px; display: flex; align-items: center; justify-content: space-between; padding: 0 18px; border: 3px solid #10132c; border-bottom: 0; border-radius: 16px 16px 0 0; background: #e9e4ff; }
  .slot { width: 60%; height: 8px; border-radius: 99px; background: #10132c; }
  .led { width: 12px; height: 12px; border: 2px solid #10132c; border-radius: 50%; background: #cfc8e8; transition: background-color 200ms; }
  .led.on { background: #28c840; animation: led 400ms steps(2) infinite; }
  @keyframes led { 50% { background: #b7f0c4; } }
  .printer-body { height: 88px; display: grid; place-items: center; border: 3px solid #10132c; border-radius: 22px; background: linear-gradient(#fff, #f1ecff); box-shadow: 0 6px 0 #10132c, 0 22px 30px #2a1b5c30; }
  .printer-body span { padding: 4px 12px; border: 2px solid #10132c; border-radius: 99px; background: #ffe66b; font-family: "HitSlop Display", sans-serif; font-size: 1rem; font-weight: 800; }
  .paper { position: absolute; z-index: 1; left: 50%; top: 112px; width: 74%; padding: 26px 22px 22px; border: 1px solid #d9d0bd; border-radius: 3px; background: #fffaf0; box-shadow: 0 20px 34px #2a1b5c26; color: #1f1c16; transform: translateX(-50%) translateY(-100%); opacity: 0; }
  .paper[data-state="printing"] { opacity: 1; animation: print 1500ms steps(12) forwards; }
  .paper[data-state="idle"], .paper[data-state="done"] { opacity: 1; transform: translateX(-50%) translateY(0) rotate(-2deg); transition: transform 500ms var(--spring); }
  @keyframes print { from { transform: translateX(-50%) translateY(-100%); } to { transform: translateX(-50%) translateY(0); } }
  .paper header { padding-bottom: 14px; display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #e3d9c4; font-family: "Newsreader", Georgia, serif; font-size: 1.5rem; }
  .paper header b { padding: 4px 9px; border-radius: 99px; color: #1d5c39; background: #c7f3dc; font-family: "HitSlop Sans", sans-serif; font-size: .75rem; }
  .paper p { margin: 18px 0 24px; font-weight: 800; }
  .paper div, .paper footer { padding: 10px 0; display: flex; justify-content: space-between; border-bottom: 1px solid #ece3d0; font-size: .95rem; }
  .paper footer { margin-top: 18px; border-top: 2px solid #1f1c16; border-bottom: 0; font-size: 1.1rem; font-weight: 800; }
  .file { position: absolute; z-index: 3; right: -12px; bottom: 16px; display: grid; gap: 3px; animation: land 520ms var(--spring); }
  .file small { color: var(--muted); font-family: "HitSlop Sans", sans-serif; font-size: .8rem; font-weight: 700; }
  @keyframes land { from { opacity: 0; transform: translateY(-40px) scale(.6) rotate(-20deg); } }
  @media (prefers-reduced-motion: reduce) { .paper[data-state="printing"], .file { animation: none; } }
</style>
