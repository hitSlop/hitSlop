import { png } from "./png-fixture";
import { cp, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

export async function definitionFixture(source: string) {
  await mkdir(join(source,"lib"),{recursive:true});
  await cp("tests/presentation/assets/washer.png",join(source,"skin.png"));
  await cp("tests/apps/assets/kalam.woff2",join(source,"font.woff2"));
  const files: Record<string,string | Buffer> = {
    "schema.ts": `import {defineDocument,s} from '@hitslop/tinys'; export default defineDocument({title:s.string()},{initial:{title:'Original'}});`,
    "commands.ts": `import {s} from '@hitslop/tinys'; import {defineCommands} from '@hitslop/tinys/commands'; import schema from './schema';
export default defineCommands(schema,{rename:{description:'Rename',args:{title:s.string()},
run({tx},args){ const sentinel='BODY_ONLY_CHANGE'; tx.fields.title.set(args.title); return sentinel; }}});`,
    "App.svelte": `<script module>if(typeof window==='undefined') throw new Error('Component evaluated without a browser');</script>
<script lang="ts">import {getDocument} from '@hitslop/tinys/svelte'; import schema from './schema'; import './theme.css'; const doc=getDocument(schema); let result=$state('');</script>
<h1>{doc.current.title}</h1><button onclick={async()=>{await doc.fields.title.set('New title');result='Accepted';}}>Run</button><output>{result}</output>
<style>h1{color:rgb(20,40,60)}</style>`,
    "Preview.svelte": `<script lang="ts">import './theme.css'; import type {CaptureProps} from "@hitslop/tinys/svelte"; import type schema from "./schema"; let {snapshot}: CaptureProps<typeof schema> = $props();</script><h1>{snapshot.title}</h1>`,
    "Icon.svelte": `<p>Fixture icon</p>`,
    "Export.svelte": `<script>import image from './export.svg';</script><img src={image} alt="Export image"/>`,
    "export.svg": '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>',
    "lib/paper.svg": '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><circle cx="5" cy="5" r="5" fill="blue"/></svg>',
    "theme.css": '@import "./font.css"; body{background-image:url("$lib/paper.svg")}',
    "font.css": '@font-face{font-family:Fixture;src:url("./font.woff2") format("woff2")}h1{font-family:Fixture}',
    "preview.png": png(1, 1),
    "unused.txt": 'not an emitted dependency',
    "tiny.ts": `import {defineTiny} from '@hitslop/tinys'; import skin from './skin.png'; import preview from './preview.png';
export default defineTiny({slug:'fixture',title:'Fixture',description:'METADATA_ONLY_SENTINEL',author:{name:'Test'},categories:['utilities'],
window:{kind:'skin',width:320,height:320,image:skin},theme:{z:'#112233',a:'#ffffff'},artwork:{preview}});`,
  };
  for (const [file,content] of Object.entries(files)) await writeFile(join(source,file),content);
  return {alias:{$lib:join(source,"lib")}};
}
