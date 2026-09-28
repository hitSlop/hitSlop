// Prove the real release-gap regression against a disposable pre-fix binding.
import {mkdir,copyFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
const root=import.meta.dir, dir=join(root,'dist/renderer-sensitivity');
await mkdir(dir,{recursive:true});
for(const name of ['renderer.test.ts','patch.ts'])await copyFile(join(root,name),join(dir,name));
const source=await Bun.file(join(root,'text-binding.ts')).text();
const start=source.indexOf('      // Input/composition may arrive while');
const end=source.indexOf('\n    });',start);
if(start<0||end<0)throw Error('Release-gap mutation target changed');
await Bun.write(join(dir,'text-binding.ts'),source.slice(0,start)+source.slice(end));
const p=Bun.spawn([process.execPath,'test','renderer.test.ts','-t','input arriving during draft release'],{cwd:dir,stdout:'pipe',stderr:'pipe'});
const [a,b,exit]=await Promise.all([new Response(p.stdout).text(),new Response(p.stderr).text(),p.exited]);
const evidence=resolve(root,'../../.hitslop/v1-evidence/hitslop-core');await mkdir(evidence,{recursive:true});
await Bun.write(join(evidence,'renderer-sensitivity.log'),a+b);
if(!exit||!(a+b).includes('release gap stalled input'))throw Error('Release-gap test failed to detect the intended regression');
console.log('Release-gap regression reproduced in disposable binding; actual source unchanged.');
