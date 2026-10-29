# Third-party licenses

| Package | License | Use |
| --- | --- | --- |
| svelte 5 | MIT | UI |
| bits-ui | MIT | language select and confirmation dialogs |
| @lucide/svelte | ISC | play, pause, delete, pen, audio and close icons |
| Lora (assets/fonts/Lora.ttf) | SIL OFL 1.1, see assets/fonts/Lora-OFL.txt | transcripts and headings |
| Space Mono Bold (assets/fonts/SpaceMono-Bold.ttf) | SIL OFL 1.1, see assets/fonts/SpaceMono-OFL.txt | the recorder display and durations |
| Barlow Condensed SemiBold (assets/fonts/BarlowCondensed-SemiBold.ttf) | SIL OFL 1.1, see assets/fonts/BarlowCondensed-OFL.txt | labels and keys |

Recording uses the browser's own `getUserMedia`, `MediaRecorder` and Web Audio; transcription is
tinys's on-device `speech.transcribe`. No network access at runtime; fonts are bundled.
