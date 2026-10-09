# hitslop

Define document schemas and edit local-first state with typed handles. Includes Svelte bindings, themes, capture, and attachments.

```sh
bun add hitslop@1.0.0 svelte@5.57.2
```

Import schema builders from `hitslop` and Svelte bindings from `hitslop/svelte`.
Recognize document errors with `isDocumentError`, `isRejected` and `isRefused`, not `instanceof`.
A command's `refuse(message)` stops it with a message the window shows the person.
Transaction handles collect writes only; previews and bound values use live handles.

See the [author guides](https://hitslop.com/docs/getting-started/) and [release guide](https://github.com/hitSlop/hitslop/blob/master/docs/guides/releasing.md).

Part of the hitSlop package 1.0.0. MIT licensed.
