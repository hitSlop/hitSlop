import { LaunchWide, LaunchVertical } from "./Launch";
import { Composition } from "remotion";
import { Film, FILM_DURATION } from "./Film";
import { Promo } from "./Promo";
import { DURATION, FPS } from "./timeline";

export const Root = () => (
  <>
    <Composition id="LaunchWide" component={LaunchWide} durationInFrames={900} fps={30} width={1920} height={1080} />
    <Composition id="LaunchVertical" component={LaunchVertical} durationInFrames={900} fps={30} width={1080} height={1920} />
    <Composition id="Promo" component={Promo} durationInFrames={DURATION} fps={FPS} width={1920} height={1080} />
    <Composition id="Film" component={Film} durationInFrames={FILM_DURATION} fps={FPS} width={1920} height={1080} />
  </>
);
