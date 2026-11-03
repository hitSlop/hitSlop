import { definePop } from "@hitslop/pop";

export default definePop({
  slug: "pocket-pod",
  title: "Pocket Pod",
  description: "Your own click-wheel player: bring local songs and YouTube links, upload a photo skin, and make it yours with stickers and colors.",
  author: { name: "pop", url: "https://dotpop.click" },
  categories: ["music", "popular"],
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
