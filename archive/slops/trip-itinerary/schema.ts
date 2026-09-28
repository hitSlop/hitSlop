import { defineDocument, s, type Value } from "@hitslop/document";

export const stopTags = ["flight", "hotel", "dining", "train", "explore"] as const;
export type StopTag = (typeof stopTags)[number];

const schema = defineDocument({
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
      done: s.boolean(),
    })),
  })),
  stubItems: s.list(s.object({
    text: s.text(),
    done: s.boolean(),
  })),
});

export type TripItinerary = Value<typeof schema.fields.node>;
export type Day = TripItinerary["days"][number];
export type Stop = Day["events"][number];
export type StubItem = TripItinerary["stubItems"][number];
export default schema;
