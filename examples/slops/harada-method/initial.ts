import type { Input } from "@hitslop/document";
import schema from "./schema";

const initial = {
  "goal": "Run my first marathon under four hours",
  "deadline": "2026-10-12",
  "themes": [
    {
      "title": "Endurance",
      "cells": [
        "Long run every Sunday",
        "Build up to 32 km",
        "Keep the easy pace easy",
        "Five running days a week",
        "One hilly route a week",
        "Log every kilometre",
        "Practise race pace late",
        "Never skip two days"
      ]
    },
    {
      "title": "Speed",
      "cells": [
        "Track intervals on Tuesday",
        "Eight by 800 m",
        "Tempo run on Thursday",
        "Strides after easy runs",
        "Learn to pace by feel",
        "A parkrun each month",
        "Hill sprints through winter",
        "Sharpen in the final month"
      ]
    },
    {
      "title": "Strength",
      "cells": [
        "Squats twice a week",
        "Single-leg work",
        "Core before bed",
        "Calf raises daily",
        "Glute bridges",
        "Keep lifting through taper",
        "Ten minutes is enough",
        "Film my form monthly"
      ]
    },
    {
      "title": "Fuel",
      "cells": [
        "Rehearse race gels",
        "Carbs the night before",
        "Drink before thirst",
        "Iron-rich meals",
        "No new food on race week",
        "Breakfast three hours out",
        "Electrolytes on long runs",
        "Eat within the hour after"
      ]
    },
    {
      "title": "Recovery",
      "cells": [
        "Eight hours of sleep",
        "A rest day is training",
        "Foam roll after long runs",
        "Massage once a month",
        "Easy week every fourth",
        "Stretch hips nightly",
        "Ice a niggle early",
        "See the physio at first pain"
      ]
    },
    {
      "title": "Kit",
      "cells": [
        "Rotate two pairs of shoes",
        "Replace them at 700 km",
        "Break in the race shoes",
        "Test kit on long runs",
        "Anti-chafe everywhere",
        "Charge the watch on Saturday",
        "Lay it all out the night before",
        "Carry one spare gel"
      ]
    },
    {
      "title": "Mind",
      "cells": [
        "Write down the why",
        "Visualise the last 10 km",
        "A mantra for the wall",
        "Race the plan, not the crowd",
        "Accept the bad runs",
        "Mark the small wins",
        "Tell people the goal",
        "Read a race report weekly"
      ]
    },
    {
      "title": "Logistics",
      "cells": [
        "Enter the race early",
        "Book travel by June",
        "Walk the start area",
        "Put the taper on a calendar",
        "Arrange the bag drop",
        "Share the schedule at home",
        "Print a pace band",
        "Sort out food for after"
      ]
    }
  ],
  "done": {
    "0:0": true,
    "0:3": true,
    "0:5": true,
    "2:2": true,
    "4:0": true,
    "4:1": true,
    "6:0": true,
    "7:0": true,
    "7:1": true
  }
};
export default initial satisfies Input<typeof schema.fields.node>;
