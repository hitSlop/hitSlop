const runtimeURL = new URL("./evaluator/hitslop_core_wasm.js", location.href).href;
// Install the handler synchronously; startup time is separate from authored execution.
const initialized = (async () => {
  const runtime = await import(/* @vite-ignore */ runtimeURL);
  await runtime.default();
  postMessage({ ready: true });
  return runtime;
})();
initialized.catch(error => postMessage({ output: null, error: String(error) }));
onmessage = async ({ data }: MessageEvent<string>) => {
  try {
    const runtime = await initialized;
    postMessage({ output: runtime.evaluateCommand(data), error: null });
  } catch (error) { postMessage({ output: null, error: String(error) }); }
};
