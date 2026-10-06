# SomaAmp

Classic Webamp radio with eight listener-supported SomaFM stations, WASM
MilkDrop, and portable Winamp skins.

```sh
bun slop dev examples/slops/soma-amp
bun slop build examples/slops/soma-amp
```

Press play to listen. VIS expands the receiver; WebGL is required and Reduce
Motion disables it. Choose **Import skin…** or drop a classic `.wsz`/ZIP. **Find skins ↗** opens
the Winamp Skin Museum in your browser; download a skin there, then import it. Skins, station,
volume, balance and EQ survive reopening. Playback starts paused. **Base** clears
the selection without deleting bytes. Modern skins and local music imports are
unsupported. Preview state is disposable; create a writable copy for persistence.

PNG/PDF export shows saved receiver settings without opening streams. SomaAmp
neither proxies nor records audio. Stream availability/CORS remain controlled by
SomaFM. Webamp is Copyright (c) 2015 Jordan Eldredge (MIT).
