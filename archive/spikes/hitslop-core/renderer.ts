import { projection } from "./patch";
import { bindText } from "./text-binding";
export async function openRust(call: (method:string,args?:any)=>Promise<any>) {
  const state = projection(await call("current"),()=>call("current"));
  const bindings = new Set<ReturnType<typeof bindText>>(), listeners = new Set<()=>void>();
  let delay = 0;
  const publish = async (reply:any) => { await state.accept(reply); for (const b of bindings) b.refresh(); for (const f of listeners) f(); return reply; };
  const host = {
    frame:state.get,
    async text(request:any) {
      const reply = await call("text",{textRequest:JSON.stringify(request)});
      if (delay) await new Promise(r=>setTimeout(r,delay));
      return publish(reply);
    },
    release:async (draft:string)=>{await call("releaseDraft",{draft});},
  };
  return {
    current:()=>state.get().value,
    subscribe(f:()=>void){listeners.add(f);return ()=>listeners.delete(f);},
    bind(element:HTMLTextAreaElement|HTMLInputElement,path:any[],report:(e:unknown)=>void) {
      const b = bindText(element,path,host,report); bindings.add(b);
      return {destroy(){b.destroy();bindings.delete(b);}};
    },
    async submit(operations:any[], frame = state.get(), currentCommand = false) {
      const value = frame.value;
      const intents = operations.map(op=> {
        if (op.type === "splice") return {type:"splice",path:op.path,...(currentCommand ? {} : {base:frame.version}),index:op.index,delete:op.deleteCount,insert:op.text};
        if (op.type === "move" && "from" in op) {
          const rows = value[op.path[0]], id = rows[op.from].$id;
          const rest = rows.filter((r:any)=>r.$id!==id);
          return {type:"move",path:op.path,id,...(rest[op.to]?{at:{before:rest[op.to].$id}}:{})};
        }
        return op;
      });
      const start = performance.now();
      const reply = await call(currentCommand ? "command" : "apply",{batch:JSON.stringify({intents})});
      const arrived = performance.now();
      await publish(reply);
      // Upper bound: includes request transport and native queueing as well as reply transfer.
      reply.patchUpperBoundMS = reply.patchBuildMS + Math.max(0,arrived-start-reply.engineMS) + performance.now()-arrived;
      return reply;
    },
    async command(operations:any[]) { return this.submit(operations, state.get(), true); },
    drain:async()=>{for(const b of bindings)await b.drain();},
    pending:()=>[...bindings].filter(b=>b.pending()).length,
    delay:(ms:number)=>{delay=ms;},
    frame:state.get,
    flush:()=>call("flush"),close:()=>call("close"),snapshot:()=>call("snapshot"),
  };
}
