// Vite turns `?url` imports into the emitted asset's URL.
declare module "*?url" {
  const url: string;
  export default url;
}
