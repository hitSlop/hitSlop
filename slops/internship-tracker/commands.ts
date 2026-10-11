import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { stages } from "./schema";
import { addApplication, addChat, removeApplication, setFollowUp, setStage, setThanked } from "./pipeline";

export default defineCommands(doc, {
  addApplication: {
    description: "Add an application at the applied stage and return its stable row ID.",
    args: {
      company: s.string({ description: "The company name; required." }),
      role: s.string({ description: "The role title; may be empty." }),
    },
    run: ({ tx }, { company, role }) => addApplication(tx, company, role),
  },
  setStage: {
    description: "Move an application to another stage by its $id.",
    args: {
      application: s.string({ description: "The application's $id." }),
      stage: s.enum(stages, { description: "The new stage." }),
    },
    run: ({ tx }, { application, stage }) => setStage(tx, application, stage),
  },
  setFollowUp: {
    description: "Set or clear an application's follow-up date by its $id.",
    args: {
      application: s.string({ description: "The application's $id." }),
      date: s.string({ description: "A local YYYY-MM-DD date, or an empty string to clear it." }),
    },
    run: ({ tx }, { application, date }) => setFollowUp(tx, application, date),
  },
  removeApplication: {
    description: "Remove an application from the badge by its $id.",
    args: { application: s.string({ description: "The application's $id." }) },
    run: ({ tx }, { application }) => removeApplication(tx, application),
  },
  addChat: {
    description: "Log a coffee chat with a person and return its stable row ID.",
    args: {
      name: s.string({ description: "The person's name; required." }),
      place: s.string({ description: "The company or place; may be empty." }),
    },
    run: ({ tx }, { name, place }) => addChat(tx, name, place),
  },
  setThanked: {
    description: "Mark a coffee chat's thank-you as sent or not, by its $id.",
    args: {
      chat: s.string({ description: "The chat's $id." }),
      thanked: s.boolean({ description: "True when the thank-you has been sent." }),
    },
    run: ({ tx }, { chat, thanked }) => setThanked(tx, chat, thanked),
  },
});
