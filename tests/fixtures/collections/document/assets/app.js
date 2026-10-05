// Plain-JS consumer of the ctx interface for records, scalar lists and optional text.
export default {
  descriptor: {
    "kind": "object",
    "properties": {
      "title": {
        "kind": "text"
      },
      "done": {
        "kind": "record",
        "value": {
          "kind": "boolean"
        }
      },
      "widths": {
        "kind": "record",
        "value": {
          "kind": "integer",
          "min": 56,
          "max": 320
        }
      },
      "cells": {
        "kind": "record",
        "value": {
          "kind": "object",
          "properties": {
            "input": {
              "kind": "string",
              "maxLength": 10
            },
            "tint": {
              "kind": "optional",
              "inner": {
                "kind": "enum",
                "values": [
                  "red",
                  "blue"
                ]
              }
            }
          }
        }
      },
      "pages": {
        "kind": "record",
        "value": {
          "kind": "object",
          "properties": {
            "text": {
              "kind": "text"
            }
          }
        }
      },
      "pixels": {
        "kind": "list",
        "item": {
          "kind": "string"
        }
      },
      "presets": {
        "kind": "list",
        "item": {
          "kind": "integer",
          "min": 40,
          "max": 240
        }
      },
      "notes": {
        "kind": "optional",
        "inner": {
          "kind": "text"
        }
      },
      "habits": {
        "kind": "list",
        "item": {
          "kind": "object",
          "properties": {
            "name": {
              "kind": "text"
            },
            "checkins": {
              "kind": "record",
              "value": {
                "kind": "integer",
                "min": 1
              }
            }
          }
        }
      },
      "stats": {
        "kind": "object",
        "properties": {
          "guesses": {
            "kind": "list",
            "item": {
              "kind": "string"
            }
          }
        }
      }
    }
  },
  mount(ctx, target) {
    const doc = ctx.document;
    const output = document.createElement("p");
    const notes = document.createElement("textarea");
    target.append(output, notes);
    const render = () => {
      const value = doc.current;
      output.textContent = `${value.title}: ${Object.keys(value.done).length} done, ${value.pixels.join(" ")}, presets ${value.presets.join("/")}`;
    };
    const stop = doc.subscribe(render);
    const binding = ctx.bind.text(notes, doc.fields.notes);
    render();
    return {
      unmount() {
        stop();
        binding.destroy();
        target.replaceChildren();
      },
    };
  },
};
