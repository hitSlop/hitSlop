import { defineDocument, s } from "@hitslop/tinys";
const checklist = defineDocument({
  title: s.text({ description: "The title shown in the window." }),
  tasks: s.list(s.object({
    text: s.text(),
    done: s.boolean({ default: false }),
    archived: s.boolean({ default: false, description: "Filed away from the to-do list." }),
  })),
}, { initial: {
    title: "Little things, today",
    tasks: [
      { text: "Send the first draft", done: true },
      { text: "Take a walk without my phone" },
      { text: "Make a little room for the weekend" },
    ],
  } });
export default checklist;
