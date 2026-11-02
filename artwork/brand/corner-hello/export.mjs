// Export the shared vector source. Chrome rasterizes SVG only; Rive previews use the CLI.
import './build.mjs';
import { readFileSync, writeFileSync, mkdirSync, copyFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { chromium } from 'playwright';
const root=dirname(fileURLToPath(import.meta.url)), repo=resolve(root,'../../..');
const rive=process.env.RIVE_CLI??'rive';
for(const args of [['--verify'],['--summary'],['--once']]) {
  const result=spawnSync(rive,args[0]==='--summary'?['inspect',root,...args]:[root,...args],{stdio:'inherit'});
  if(result.status!==0)throw new Error('Rive verification/export failed');
}
const resources=join(repo,'apps/apple/Packages/PopApple/Sources/PopLaunch/Resources');
const native=join(repo,'apps/apple/Packages/PopApple/Sources/PopCore/Resources/Brand.xcassets/BrandCharacter.imageset');
const web=join(repo,'apps/landing/public/assets/brand');
mkdirSync(native,{recursive:true});mkdirSync(resources,{recursive:true});
copyFileSync(join(root,'build/corner-hello.riv'),join(resources,'corner-hello.riv'));
copyFileSync(join(root,'mark.svg'),join(web,'mark.svg'));
copyFileSync(join(root,'hello.svg'),join(web,'hello.svg'));
const browser=await chromium.launch({channel:'chrome',headless:true});
try {
  const page=await browser.newPage({viewport:{width:512,height:512},deviceScaleFactor:1});
  const mark=readFileSync(join(root,'mark.svg'),'utf8'),hello=readFileSync(join(root,'hello.svg'),'utf8');
  async function render(art,width=512,height=512) {
    await page.setViewportSize({width,height});
    await page.setContent('<style>*{box-sizing:border-box}html,body{margin:0;width:100%;height:100%;background:transparent}svg{width:100%;height:100%;display:block}</style>'+art);
    return await page.screenshot({omitBackground:true});
  }
  function webp(png,name) {
    const result=spawnSync('magick',['png:-','-quality','92',join(web,name)],{input:png});
    if(result.status!==0)throw new Error(String(result.stderr));
  }
  const png=await render(mark);
  writeFileSync(join(native,'character-512.png'),png);
  writeFileSync(join(native,'Contents.json'),JSON.stringify({images:[{filename:'character-512.png',idiom:'universal'}],info:{author:'xcode',version:1}},null,2)+'\n');
  webp(png,'mark.webp');
  const greeting=await render(hello);
  webp(greeting,'mascot-wave.webp');webp(greeting,'mascot-party.webp');
  const wordmark=readFileSync(join(web,'wordmark.webp')).toString('base64');
  const logo=await render('<div style="height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px"><div style="width:390px;height:430px">'+mark+'</div><img style="width:370px;height:auto" src="data:image/webp;base64,'+wordmark+'"></div>',512,620);
  webp(logo,'logo.webp');
  const converted=spawnSync('magick',[join(web,'wordmark.webp'),join(resources,'wordmark.png')]);
  if(converted.status!==0)throw new Error('Wordmark export failed');
} finally { await browser.close(); }
console.log('Exported the shared native, Rive, and website artwork.');
