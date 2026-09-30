<script lang="ts">
import { useDocument } from "@hitslop/document/svelte";
import schema from "./schema";
const doc = useDocument(schema);
let { mode }: { mode: "preview" | "export" } = $props();
</script>

<main><h1>{doc.current.title}</h1><p>{doc.current.rows.length} rows</p></main>
