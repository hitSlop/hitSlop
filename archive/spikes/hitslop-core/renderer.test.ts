// Failures: a stale/gapped publication corrupts a view; one row edit recreates
// every row; delayed/rejected input overwrites a DOM draft. These are renderer
// contracts, independent of Rust's semantic fixture suite.
import {test,expect} from 'bun:test';
import {applyOps,projection} from './patch';
import {bindText,splice} from './text-binding';
test('identity patches preserve unaffected rows and apply one move',()=>{
 const a={$id:'a',done:false},b={$id:'b',done:false};const before={rows:[a,b]};
 const after=applyOps(before,[{type:'set',path:['rows',{id:'a'},'done'],value:true}]);
 expect(after.rows[0].done).toBe(true);expect(after.rows[1]).toBe(b);expect(before.rows[0].done).toBe(false);
 const moved=applyOps(after,[{type:'moveRow',path:['rows'],id:'a',index:1}]);expect(moved.rows).toEqual([b,after.rows[0]]);expect(moved.rows[1]).toBe(after.rows[0]);
});
test('stale replies are ignored and gaps/session changes get a fresh snapshot',async()=>{
 let gets=0;const fresh={session:'s',sequence:3,version:'v3',value:{title:'fresh'},issues:[]};
 const p=projection({...fresh,sequence:1,value:{title:'old'}},async()=>{gets++;return fresh;});
 await p.accept({patch:{session:'s',previous:0,sequence:1,ops:[{type:'set',path:['title'],value:'stale'}],issues:[]}});expect(p.get().value.title).toBe('old');
 await p.accept({patch:{session:'s',previous:2,sequence:3,ops:[],issues:[]}});expect(gets).toBe(1);expect(p.get().value.title).toBe('fresh');
 await p.accept({patch:{session:'previous-owner',previous:3,sequence:4,ops:[]}});expect(gets).toBe(2);
});
test('UTF16 splice never divides an emoji shared high surrogate',()=>{
 expect(splice('a😀é','a🚀é')).toEqual({index:1,delete:2,insert:'🚀'});
});
class Field extends EventTarget {value='';selectionStart=0;selectionEnd=0;blurred=false;setSelectionRange(a:number,b:number){this.selectionStart=a;this.selectionEnd=b;}blur(){this.blurred=true;}}
test('composition stays local and a failed submission retains the draft',async()=>{
 const field=new Field();let submissions=0;const errors:unknown[]=[];
 const b=bindText(field as any,['title'],{frame:()=>({session:'s',version:'v',value:{title:'abc'}}),text:async()=>{submissions++;throw Error('disk/transport');},release:async()=>{}},e=>errors.push(e));
 field.dispatchEvent(new Event('compositionstart'));field.value='abc日本';field.dispatchEvent(new Event('input'));expect(submissions).toBe(0);
 field.dispatchEvent(new Event('compositionend'));await expect(b.drain()).rejects.toThrow('disk/transport');expect(submissions).toBe(1);expect(field.value).toBe('abc日本');expect(errors).toHaveLength(1);b.destroy();
});
test('input arriving during draft release is sent and drain waits for it',async()=>{
 const field=new Field();let title='a', calls=0, nextAccepted!:()=>void, releaseStarted!:()=>void, releaseDone!:()=>void;
 const next=new Promise<void>(r=>nextAccepted=r);
 const releasing=new Promise<void>(r=>releaseStarted=r), gate=new Promise<void>(r=>releaseDone=r);
 const b=bindText(field as any,['title'],{
  frame:()=>({session:'s',version:'v',value:{title}}),
  text:async r=>{calls++;if(calls===2)nextAccepted();title=title.slice(0,r.index)+r.insert+title.slice(r.index+r.delete);return{text:{selectionStart:title.length,selectionEnd:title.length}};},
  release:async()=>{if(calls===1){releaseStarted();await gate;}},
 });
 field.value='ab';field.setSelectionRange(2,2);field.dispatchEvent(new Event('input'));await releasing;
 field.value='abc';field.setSelectionRange(3,3);field.dispatchEvent(new Event('input'));releaseDone();
 let timeout:ReturnType<typeof setTimeout>;
 try { await Promise.race([next,new Promise((_,reject)=>{timeout=setTimeout(()=>reject(Error('release gap stalled input')),500);})]); }
 finally {clearTimeout(timeout!);}
 await b.drain();expect(title).toBe('abc');expect(calls).toBe(2);expect(b.pending()).toBe(false);b.destroy();
});
test('a delayed gap snapshot cannot overwrite a newer publication',async()=>{
 let finish!:(frame:any)=>void;
 const p=projection({session:'s',sequence:1,version:'v1',value:{title:'one'},issues:[]},()=>new Promise(r=>finish=r));
 const gap=p.accept({version:'v3',patch:{session:'s',previous:2,sequence:3,ops:[],issues:[]}});
 await p.accept({version:'v2',patch:{session:'s',previous:1,sequence:2,ops:[{type:'set',path:['title'],value:'two'}],issues:[]}});
 await p.accept({version:'v3',patch:{session:'s',previous:2,sequence:3,ops:[{type:'set',path:['title'],value:'three'}],issues:[]}});
 finish({session:'s',sequence:2,version:'v2',value:{title:'two'},issues:[]});await gap;
 expect(p.get().value.title).toBe('three');expect(p.get().sequence).toBe(3);
});
