import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  title: "Departures",
  rows: [
    { time: "07:30", text: "COFFEE RUN", status: "gone" },
    { time: "09:00", text: "DEEP WORK", status: "boarding" },
    { time: "12:30", text: "LUNCH W/ MAYA", status: "on-time" },
    { time: "15:00", text: "DENTIST", status: "delayed" },
    { time: "18:45", text: "PICK UP BIKE", status: "on-time" },
    { time: "21:00", text: "MOVIE NIGHT", status: "on-time" },
  ],
} satisfies Input<typeof schema.descriptor>;
