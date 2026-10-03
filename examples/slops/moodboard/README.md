# Moodboard

Drop in images and fly through them in a soft 3D tunnel, with captions underneath. Images are stored by hitSlop, not in the document.

The tunnel is the Infinite Gallery from [Motion Core](https://github.com/kaltwrk/motion-core) (MIT, see `motion-core/LICENSE`) on [OGL](https://github.com/oframe/ogl) (MIT), and the Add button uses Motion Core's Magnetic with [GSAP](https://gsap.com) (free under the GSAP Standard License). The copied files list their changes at the top.

From the repository root:

```sh
bun slop dev examples/slops/moodboard
bun slop build examples/slops/moodboard
bun slop register examples/slops/moodboard
```

Create a writable copy of the registered template before editing.
