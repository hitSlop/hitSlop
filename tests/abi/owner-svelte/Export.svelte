<script lang="ts">
import doc from "./schema";
let { mode }: { mode: "preview" | "export" } = $props();
</script>

<main><h1>{doc.current.title}</h1><p>{doc.current.rows.length} rows</p></main>
