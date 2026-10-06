import CloudRain from "@lucide/svelte/icons/cloud-rain";
import CloudLightning from "@lucide/svelte/icons/cloud-lightning";
import Wind from "@lucide/svelte/icons/wind";
import Bird from "@lucide/svelte/icons/bird";
import Moon from "@lucide/svelte/icons/moon";

export const CHANNELS = [
  { id: "rain" as const, label: "Rain", icon: CloudRain },
  { id: "thunder" as const, label: "Thunder", icon: CloudLightning },
  { id: "wind" as const, label: "Wind", icon: Wind },
  { id: "birds" as const, label: "Birds", icon: Bird },
  { id: "night" as const, label: "Night", icon: Moon },
];
