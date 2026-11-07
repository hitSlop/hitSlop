import { definePop } from "@hitslop/pop";
export default definePop({
  slug: "focus-timer",
  title: "Pomodoro",
  description: "A cheerful tomato timer. A little time for you.",
  author: { name: "pop", url: "https://dotpop.click" },
  categories: ["productivity"],
  window: { kind: "standard", width: 320, height: 320, shape: "50%", lockAspect: true },
  theme: {
    surface: "#ee4c3f",
    surfaceLight: "#ff7862",
    surfaceDeep: "#be352d",
    paper: "#fff2d3",
    ink: "#342b24",
  },
});
