<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  // A steno pad with the recorder's red key. Independent of saved state and devices.
  import Spiral from "./Spiral.svelte";
</script>

<div class="vn-icon" aria-hidden="true">
  <div class="vn-icon-pad">
    <Spiral rings={5} />
    <div class="vn-icon-lines"><i></i><i></i><i></i><i></i></div>
  </div>
  <div class="vn-icon-key"><span></span></div>
</div>
