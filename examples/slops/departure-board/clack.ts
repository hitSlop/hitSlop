// A short filtered-noise tick for each flip, only after the person turns sound on.
let context: AudioContext | undefined;
let noise: AudioBuffer | undefined;
let last = 0;

export function enableSound(): void {
  if (context) return;
  const Ctor = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
  context = new Ctor();
  void context.resume();
  noise = context.createBuffer(1, Math.floor(context.sampleRate * 0.03), context.sampleRate);
  const data = noise.getChannelData(0);
  for (let i = 0; i < data.length; i++) data[i] = (Math.random() * 2 - 1) * (1 - i / data.length);
}

export function disableSound(): void {
  void context?.close();
  context = undefined;
  noise = undefined;
}

export function clack(): void {
  if (!context || !noise || context.state !== "running") return;
  const now = context.currentTime;
  if (now - last < 0.035) return;
  last = now;
  const source = context.createBufferSource();
  source.buffer = noise;
  const filter = context.createBiquadFilter();
  filter.type = "bandpass";
  filter.frequency.value = 1800 + Math.random() * 900;
  const gain = context.createGain();
  gain.gain.value = 0.18;
  source.connect(filter).connect(gain).connect(context.destination);
  source.start(now);
}
