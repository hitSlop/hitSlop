import { png } from "./png-fixture";
import { cp, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

export async function definitionFixture(source: string) {
  await mkdir(join(source,"lib"),{recursive:true});
  await cp("tests/presentation/assets/washer.png",join(source,"skin.png"));
  await cp("apps/landing/public/assets/hero/fonts/newsreader-latin.woff2",join(source,"font.woff2"));
  const files: Record<string,string | Buffer> = {
    "model.ts": `import {defineDocument,s} from 'hitslop'; export default defineDocument({title:s.string()});`,
    "actions.ts": `import {s} from 'hitslop'; import doc from './model';
export const rename = doc.command({description:'Rename',args:{title:s.string()},
run({tx},args){ const sentinel='BODY_ONLY_CHANGE'; tx.fields.title.set(args.title); return sentinel; }});`,
    "Room.svelte": `<script module>if(typeof window==='undefined') throw new Error('Component evaluated without a browser');</script>
<script lang="ts">import doc from './model'; import {rename} from './actions'; let result=$state('');</script>
<h1>{doc.current.title}</h1><button onclick={async()=>{result=String(await rename({title:'New title'}));}}>Run</button><output>{result}</output>
<style>h1{color:rgb(20,40,60)}</style>`,
    "Print.svelte": `<script>import image from './export.svg';</script><img src={image} alt="Export image"/>`,
    "export.svg": '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>',
    "lib/paper.svg": '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><circle cx="5" cy="5" r="5" fill="blue"/></svg>',
    "theme.css": '@import "./font.css"; body{background-image:url("$lib/paper.svg")}',
    "font.css": '@font-face{font-family:Fixture;src:url("./font.woff2") format("woff2")}h1{font-family:Fixture}',
    "preview.png": png(1, 1),
    "unused.txt": 'not an emitted dependency',
    "slop.ts": `import {defineSlop} from 'hitslop'; import Room from './Room.svelte'; import Print from './Print.svelte';
import document from './model'; import {rename} from './actions'; import skin from './skin.png'; import preview from './preview.png'; import './theme.css';
export default defineSlop({slug:'fixture',title:'Fixture',description:'Build gate',author:{name:'Test'},categories:['utilities'],
window:{kind:'skin',width:320,height:320,image:skin},theme:{z:'#112233',a:'#ffffff'},document,initial:{title:'Original'},view:Room,export:Print,
commands:{rename},artwork:{preview}});`,
  };
  for (const [file,content] of Object.entries(files)) await writeFile(join(source,file),content);
  return {alias:{$lib:join(source,"lib")}};
}
