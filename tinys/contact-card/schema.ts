import { defineDocument, s } from "@hitslop/tinys";

export default defineDocument({
  name: s.text(),
  headline: s.text(),
  location: s.text(),
  bio: s.text(),
  email: s.string(),
  phone: s.string(),
  website: s.string(),
  avatar: s.optional(s.object({ id: s.string(), mimeType: s.string() })),
}, { initial: {
  name: "Jamie Park",
  headline: "Freelance Product Designer",
  location: "San Francisco, CA",
  bio: "I design thoughtful products and tactile digital tools that solve real everyday problems.",
  email: "jamie@park.dev",
  phone: "(555) 987-6543",
  website: "https://park.dev",
} });
