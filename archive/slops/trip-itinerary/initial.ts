import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  "tripTitle": "TOKYO AUTUMN TRIP",
  "origin": "SFO",
  "originCity": "SAN FRANCISCO",
  "destination": "NRT",
  "destCity": "TOKYO NARITA",
  "bookingRef": "HS-8842",
  "passenger": "JORDAN / PASSENGER",
  "flight": "HS-774",
  "gate": "B12",
  "seat": "02A",
  "selectedDay": "d-1",
  "days": [
    {
      "dayKey": "d-1",
      "title": "DAY 01",
      "subtitle": "Departure & Arrival",
      "date": "2026-10-14",
      "events": [
        {
          "time": "11:30",
          "title": "Flight HS-774 Departure",
          "location": "SFO Terminal 3 Gate 82",
          "tag": "flight",
          "done": true
        },
        {
          "time": "15:45",
          "title": "Touchdown Tokyo Narita",
          "location": "NRT Terminal 1",
          "tag": "flight",
          "done": true
        },
        {
          "time": "17:15",
          "title": "Narita Express to Shibuya",
          "location": "Platform 1",
          "tag": "train",
          "done": false
        },
        {
          "time": "19:00",
          "title": "Hotel Check-in",
          "location": "Shibuya Stream Hotel",
          "tag": "hotel",
          "done": false
        },
        {
          "time": "20:30",
          "title": "Late Ramen Supper",
          "location": "Ichiran Shibuya",
          "tag": "dining",
          "done": false
        }
      ]
    },
    {
      "dayKey": "d-2",
      "title": "DAY 02",
      "subtitle": "Meiji & Omotesando",
      "date": "2026-10-15",
      "events": [
        {
          "time": "08:30",
          "title": "Morning walk at Meiji Jingu",
          "location": "Harajuku Gate",
          "tag": "explore",
          "done": false
        },
        {
          "time": "11:00",
          "title": "Coffee & Pastries",
          "location": "Chatei Hatou",
          "tag": "dining",
          "done": false
        },
        {
          "time": "14:00",
          "title": "Architecture & Boutiques",
          "location": "Omotesando Hills",
          "tag": "explore",
          "done": false
        },
        {
          "time": "18:30",
          "title": "Yakitori Dinner",
          "location": "Omoide Yokocho",
          "tag": "dining",
          "done": false
        }
      ]
    },
    {
      "dayKey": "d-3",
      "title": "DAY 03",
      "subtitle": "Old Asakusa & Akihabara",
      "date": "2026-10-16",
      "events": [
        {
          "time": "09:00",
          "title": "Senso-ji Temple grounds",
          "location": "Asakusa",
          "tag": "explore",
          "done": false
        },
        {
          "time": "13:00",
          "title": "Retro Electronics & Arcades",
          "location": "Akihabara Electric Town",
          "tag": "explore",
          "done": false
        },
        {
          "time": "19:30",
          "title": "Tonkatsu Dinner",
          "location": "Katsukura",
          "tag": "dining",
          "done": false
        }
      ]
    }
  ],
  "stubItems": [
    {
      "text": "Passport & Visas",
      "done": true
    },
    {
      "text": "eSIM Activated",
      "done": true
    },
    {
      "text": "Power Adapters",
      "done": true
    },
    {
      "text": "Suica / IC Card loaded",
      "done": false
    },
    {
      "text": "Pocket Wi-Fi picked up",
      "done": false
    }
  ]
} satisfies Input<typeof schema.fields.node>;
