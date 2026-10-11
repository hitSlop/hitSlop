# Third-party licenses

Koi Pond bundles two libraries into its app script. Their full license texts live in
`assets/`; `pond.ts` imports them, so they are compiled into the app script that every
Koi Pond document carries (the pond's hidden painter element holds them at runtime).

| Package | Version | License | Text |
| --- | --- | --- | --- |
| [p5](https://github.com/processing/p5.js) | 2.3.3 | LGPL-2.1 | `assets/p5-LICENSE.txt` |
| [p5.brush](https://github.com/acamposuribe/p5.brush) | 2.2.3 | MIT | `assets/p5.brush-LICENSE.txt` |

p5 is used unmodified, as published on npm.
