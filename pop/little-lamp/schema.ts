import { defineDocument, s } from "@hitslop/pop";

export default defineDocument({
  on: s.boolean({ default: false, description: "Whether the little lamp is switched on." }),
});
