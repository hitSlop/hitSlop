import { defineDocument, s } from "hitslop";

export const stopTags = ["flight", "hotel", "dining", "train", "explore"] as const;

export default defineDocument({
  tripTitle: s.text(),
  origin: s.text(),
  originCity: s.text(),
  destination: s.text(),
  destCity: s.text(),
  bookingRef: s.text(),
  passenger: s.text(),
  flight: s.text(),
  gate: s.text(),
  seat: s.text(),
  selectedDay: s.string(),
  days: s.list(s.object({
    dayKey: s.string(),
    title: s.text(),
    subtitle: s.text(),
    date: s.string(),
    events: s.list(s.object({
      time: s.string(),
      title: s.text(),
      location: s.text(),
      tag: s.enum(stopTags),
      done: s.boolean({ default: false }),
    })),
  })),
  stubItems: s.list(s.object({
    text: s.text(),
    done: s.boolean({ default: false }),
  })),
}, { initial: {
  tripTitle: "TOKYO AUTUMN TRIP",
  origin: "SFO",
  originCity: "SAN FRANCISCO",
  destination: "NRT",
  destCity: "TOKYO NARITA",
  bookingRef: "HS-8842",
  passenger: "JORDAN / PASSENGER",
  flight: "HS-774",
  gate: "B12",
  seat: "02A",
  selectedDay: "d-1",
  days: [
    {
      dayKey: "d-1",
      title: "DAY 01",
      subtitle: "Departure & Arrival",
      date: "2026-10-14",
      events: [
        { time: "11:30", title: "Flight HS-774 Departure", location: "SFO Terminal 3 Gate 82", tag: "flight", done: true },
        { time: "15:45", title: "Touchdown Tokyo Narita", location: "NRT Terminal 1", tag: "flight", done: true },
        { time: "17:15", title: "Narita Express to Shibuya", location: "Platform 1", tag: "train" },
        { time: "19:00", title: "Hotel Check-in", location: "Shibuya Stream Hotel", tag: "hotel" },
        { time: "20:30", title: "Late Ramen Supper", location: "Ichiran Shibuya", tag: "dining" },
      ],
    },
    {
      dayKey: "d-2",
      title: "DAY 02",
      subtitle: "Meiji & Omotesando",
      date: "2026-10-15",
      events: [
        { time: "08:30", title: "Morning walk at Meiji Jingu", location: "Harajuku Gate", tag: "explore" },
        { time: "11:00", title: "Coffee & Pastries", location: "Chatei Hatou", tag: "dining" },
        { time: "14:00", title: "Architecture & Boutiques", location: "Omotesando Hills", tag: "explore" },
        { time: "18:30", title: "Yakitori Dinner", location: "Omoide Yokocho", tag: "dining" },
      ],
    },
    {
      dayKey: "d-3",
      title: "DAY 03",
      subtitle: "Old Asakusa & Akihabara",
      date: "2026-10-16",
      events: [
        { time: "09:00", title: "Senso-ji Temple grounds", location: "Asakusa", tag: "explore" },
        { time: "13:00", title: "Retro Electronics & Arcades", location: "Akihabara Electric Town", tag: "explore" },
        { time: "19:30", title: "Tonkatsu Dinner", location: "Katsukura", tag: "dining" },
      ],
    },
  ],
  stubItems: [
    { text: "Passport & Visas", done: true },
    { text: "eSIM Activated", done: true },
    { text: "Power Adapters", done: true },
    { text: "Suica / IC Card loaded" },
    { text: "Pocket Wi-Fi picked up" },
  ],
} });
