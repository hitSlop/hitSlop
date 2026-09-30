<script lang="ts">

function checkRender(kind: "editor" | "export" | "icon") {
    const failure = (globalThis as any).__presentationFailure;
    if (failure?.kind === kind) throw failure.error;
    return "";
  }
</script>

<div class="icon">{checkRender("icon")}</div>
