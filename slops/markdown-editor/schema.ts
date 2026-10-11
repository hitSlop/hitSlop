import { defineDocument, s } from "hitslop";

export const modes = ["inplace", "split", "preview"] as const;
export const themes = ["paper", "dark"] as const;

export default defineDocument({
  title: s.text({ description: "The manuscript title." }),
  content: s.text({ description: "The markdown source." }),
  mode: s.enum(modes, { description: "inplace edits over the ink, split shows source and page, preview shows the page." }),
  theme: s.enum(themes, { description: "Paper or dark carbon." }),
}, { initial: {
  title: "On writing in place",
  content: "# On writing in place\n\nThe page is just a **native textarea** sitting over the ink.\n\nType. The marks stay. Undo works. The caret is real.\n\n## Why a typewriter\n1. Native undo, redo, and selection\n2. The keyboard you already know\n3. No contentEditable surprises\n\n> Simple request: edit markdown. Reality: install fifty dependencies.\n\n## Shortcuts\n- ⌘B or Ctrl+B for **bold**\n- ⌘I or Ctrl+I for *italic*\n- ⌘E or Ctrl+E to cycle Edit, Split, and Preview\n\n`const page = \"just a textarea\"`\n",
  mode: "inplace",
  theme: "paper",
} });
