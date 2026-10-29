import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "pocket-pod",
  title: "Pocket Pod",
  description: "A pocket click-wheel player: paste YouTube links, spin through them in Cover Flow, and cover the case in stickers.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["music", "media"],
  window: { kind: "standard", width: 360, height: 603, shape: "30px", lockAspect: true },
  theme: {
    case: "#c9ccd1",
    caseHi: "#eceef1",
    caseLo: "#9a9ea6",
    bezel: "#17181a",
    screen: "#fbfcfd",
    screenInk: "#0d0e10",
    accent: "#2f7de1",
    wheel: "#f4f4f2",
    wheelInk: "#7b808a",
    button: "#d6d9de",
  },
});
