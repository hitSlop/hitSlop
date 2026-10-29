import { defineDocument, s } from "@hitslop/tinys";
import { statuses, widths } from "./board";

export default defineDocument({
  title: s.string({ maxLength: widths.title, description: "The board's heading." }),
  rows: s.list(s.object({
    time: s.string({ maxLength: widths.time, description: "Departure time, such as 09:00." }),
    text: s.string({ maxLength: widths.text, description: "Where the stop goes, upper case." }),
    status: s.enum(statuses, { default: "on-time" }),
  })),
}, { initial: {
  title: "Departures",
  rows: [
    { time: "07:30", text: "COFFEE RUN", status: "gone" },
    { time: "09:00", text: "DEEP WORK", status: "boarding" },
    { time: "12:30", text: "LUNCH W/ MAYA" },
    { time: "15:00", text: "DENTIST", status: "delayed" },
    { time: "18:45", text: "PICK UP BIKE" },
    { time: "21:00", text: "MOVIE NIGHT" },
  ],
} });
