declare module "webamp/butterchurn" {
  import Webamp from "webamp";
  export default Webamp;
}
/** Native host global for unskinned windows; absent in slop dev and capture. */
declare var slop: { window: { resize(size: { width: number; height: number }): Promise<{ width: number; height: number }> } } | undefined;
