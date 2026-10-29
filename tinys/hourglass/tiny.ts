import { defineTiny } from "@hitslop/tinys";
import { height, silhouette, width } from "./glass";

export default defineTiny({
  slug: "hourglass",
  title: "Hourglass",
  description: "A frosted sand glass that drains toward a time you choose.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["productivity", "personal"],
  window: { kind: "standard", fullscreenable: true,
    width,
    height,
    shape: { path: silhouette, viewBox: [width, height] },
    lockAspect: true,
    background: "glass",
  },
  theme: {
    glass: "#fff8ec5c",
    sand: "#e2b15e",
    frame: "#4b2f1f",
    onFrame: "#f4e7d2",
    ink: "#2a1f17",
  },
});
