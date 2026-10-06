# @hitslop/document

Define document schemas and edit local-first state with typed handles. Includes Svelte bindings, themes, capture, and attachments.

```sh
bun add @hitslop/document@4.0.0 svelte@5.57.1
```

Import schema builders from `@hitslop/document` and Svelte bindings from `@hitslop/document/svelte`.
Recognize document errors with `isDocumentError` and `isRejected`, not `instanceof`.
Transaction handles collect writes only; previews and bound values use live handles.

See the [author guides](https://hitslop.com/docs/getting-started/) and [release guide](https://github.com/hitSlop/hitslop/blob/master/docs/guides/releasing.md).

Part of the hitSlop SDK 4.0.0. MIT licensed.
