import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  name: "Jamie Park",
  headline: "Freelance Product Designer",
  location: "San Francisco, CA",
  bio: "I design thoughtful products and tactile digital tools that solve real everyday problems.",
  email: "jamie@park.dev",
  phone: "(555) 987-6543",
  website: "https://park.dev",
} satisfies Input<typeof schema.fields.node>;
