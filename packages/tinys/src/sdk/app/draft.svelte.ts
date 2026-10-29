/**
 * The text of a composer: `const task = draft(text => addTask({ text }))`.
 * Bind an input to `task.value`, disable the button unless
 * `task.ready`, and call `task.submit` from the form. The markup is yours.
 */
export function draft<R>(submit: (text: string) => Promise<R>) {
  let value = $state("");
  let pending = $state(false);
  return {
    get value() {
      return value;
    },
    set value(next: string) {
      value = next;
    },
    /** A submission is running. */
    get pending() {
      return pending;
    },
    /** There is text to submit and no submission is running. */
    get ready() {
      return !pending && value.trim() !== "";
    },
    /** Runs the command with the trimmed text and resolves to its result. The text clears
     * once accepted, unless the person typed more meanwhile; a refusal keeps it. Empty text
     * and a second submit while one runs do nothing and resolve to `undefined`. */
    async submit(event?: Event): Promise<R | undefined> {
      event?.preventDefault();
      const submitted = value;
      const text = submitted.trim();
      if (!text || pending) return undefined;
      pending = true;
      try {
        const result = await submit(text);
        if (value === submitted) value = "";
        return result;
      } finally {
        pending = false;
      }
    },
  };
}
