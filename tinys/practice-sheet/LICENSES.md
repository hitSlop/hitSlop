# Third-party software and fonts in Practice Sheet

| Package / font | Version | License | Used for |
| --- | --- | --- | --- |
| abcjs | 6.7.1 | MIT (Copyright (c) 2009-2026 Paul Rosen and Gregory Dyke) | Parsing and engraving ABC; note timings |
| bits-ui | workspace pin (`^2.19.5`) | MIT | Toggle and Popover primitives |
| @lucide/svelte | 1.52.0 | ISC | Icons |
| svelte | 5.57.2 | MIT | UI framework |
| Fraunces (variable) | from Google Fonts | SIL OFL 1.1, `assets/fonts/Fraunces-OFL.txt` | Titles, numerals, body |
| Barlow Condensed SemiBold | from Google Fonts | SIL OFL 1.1, `assets/fonts/Barlow-OFL.txt` | Labels and controls |

No soundfont or audio samples are bundled; all sound is synthesized in `player.ts`.
The sample piece, Minuet in G (attributed to Christian Petzold, c. 1725), is in the
public domain; its ABC transcription here was written for this tiny.

Note: the minified `ui.js` does not keep abcjs's license comment; the notice is recorded
here in the source.
