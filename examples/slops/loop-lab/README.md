# Loop Lab

Layer loops written in [Strudel](https://strudel.cc)'s mini-notation. Each track is a pattern like `bd*2 ~ sd` (drums) or `c3 <e3 g3>` (a synth line) with volume, filter and reverb; the tracks play together at the tempo you set.

**License:** this slop includes Strudel (`@strudel/core`, `@strudel/mini`, `@strudel/webaudio`, `superdough`), which is licensed AGPL-3.0-or-later. Unlike the rest of hitSlop (MIT), **this template is therefore AGPL-3.0-or-later** (see `LICENSE`), and so is anything you build by copying it. Its source is this directory. Drum sounds are fetched when you press Play from the public `tidalcycles/dirt-samples` repository; synth voices need no network.

The slop does not evaluate free-form code (the app's content security policy blocks `eval`); it builds Strudel patterns from the notation you type.

From the repository root:

```sh
bun slop dev examples/slops/loop-lab
bun slop build examples/slops/loop-lab
bun slop register examples/slops/loop-lab
```

Create a writable copy of the registered template before editing.
