import { defineDocument, s } from 'hitslop';
export default defineDocument({rows:s.list(s.object({text:s.text()})),files:s.list(s.object({hash:s.string(),name:s.string(),mimeType:s.string(),bytes:s.integer()}))},{initial:{rows:[],files:[]}});
