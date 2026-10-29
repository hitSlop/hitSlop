// A short filtered noise burst, like the tick of the wheel. Created lazily after a user gesture.
let context: AudioContext | null = null;
let burst: AudioBuffer | null = null;

export function click(): void {
  try {
    context ??= new AudioContext();
    if (context.state === "suspended") void context.resume();
    if (!burst) {
      const length = Math.floor(context.sampleRate * 0.012);
      burst = context.createBuffer(1, length, context.sampleRate);
      const data = burst.getChannelData(0);
      for (let i = 0; i < length; i += 1) data[i] = (Math.random() * 2 - 1) * (1 - i / length) ** 6;
    }
    const source = context.createBufferSource();
    source.buffer = burst;
    const filter = context.createBiquadFilter();
    filter.type = "bandpass";
    filter.frequency.value = 2600;
    filter.Q.value = 0.9;
    const gain = context.createGain();
    gain.gain.value = 0.25;
    source.connect(filter).connect(gain).connect(context.destination);
    source.start();
  } catch {
    // No audio device: the wheel simply stays quiet.
  }
}

export function closeClicker(): void {
  void context?.close();
  context = null;
  burst = null;
}
