import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema from "./schema";
import { addEducation, addExperience, addSkill, removeEducation, removeExperience, removeSkill } from "./entries";

export default defineCommands(schema, {
  addSkill: {
    description: "Add a skill with placeholder text and return its stable row ID.",
    args: {},
    run: ({ tx }) => addSkill(tx),
  },
  removeSkill: {
    description: "Remove a skill from the resume.",
    args: { skill: s.string({ description: "The skill's $id." }) },
    run: ({ tx }, { skill }) => removeSkill(tx, skill),
  },
  addExperience: {
    description: "Add an experience entry with placeholder text and return its stable row ID.",
    args: {},
    run: ({ tx }) => addExperience(tx),
  },
  removeExperience: {
    description: "Remove an experience entry from the resume.",
    args: { experience: s.string({ description: "The experience entry's $id." }) },
    run: ({ tx }, { experience }) => removeExperience(tx, experience),
  },
  addEducation: {
    description: "Add an education entry with placeholder text and return its stable row ID.",
    args: {},
    run: ({ tx }) => addEducation(tx),
  },
  removeEducation: {
    description: "Remove an education entry from the resume.",
    args: { education: s.string({ description: "The education entry's $id." }) },
    run: ({ tx }, { education }) => removeEducation(tx, education),
  },
});
