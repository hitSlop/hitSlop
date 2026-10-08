// Plain-JS consumer of the ctx interface for the scalar kinds. No engine or bridge imports.
export default {
  descriptor: {
    "kind": "object",
    "properties": {
      "title": {
        "kind": "text"
      },
      "currency": {
        "kind": "enum",
        "values": [
          "CAD",
          "USD",
          "EUR"
        ]
      },
      "label": {
        "kind": "string",
        "maxLength": 4
      },
      "ratio": {
        "kind": "number",
        "min": 0,
        "max": 1
      },
      "rating": {
        "kind": "integer",
        "min": 1,
        "max": 5
      },
      "memo": {
        "kind": "optional",
        "inner": {
          "kind": "string"
        }
      },
      "limit": {
        "kind": "optional",
        "inner": {
          "kind": "integer",
          "min": 0,
          "max": 999
        }
      },
      "photo": {
        "kind": "optional",
        "inner": {
          "kind": "object",
          "properties": {
            "id": {
              "kind": "string"
            },
            "name": {
              "kind": "string"
            }
          }
        }
      },
      "box": {
        "kind": "optional",
        "inner": {
          "kind": "object",
          "properties": {
            "items": {
              "kind": "list",
              "item": {
                "kind": "object",
                "properties": {
                  "done": {
                    "kind": "boolean"
                  }
                }
              }
            }
          }
        }
      },
      "rows": {
        "kind": "list",
        "item": {
          "kind": "object",
          "properties": {
            "text": {
              "kind": "text"
            },
            "amount": {
              "kind": "number"
            },
            "note": {
              "kind": "optional",
              "inner": {
                "kind": "string"
              }
            }
          }
        }
      }
    }
  },
  mount(ctx, target) {
    const doc = ctx.document;
    const output = document.createElement("p");
    const ratio = Object.assign(document.createElement("input"), { type: "range", min: "0", max: "1", step: "0.05" });
    const currency = document.createElement("select");
    for (const value of ["CAD", "USD", "EUR"]) currency.append(new Option(value, value));
    target.append(output, ratio, currency);
    const render = () => {
      const value = doc.current;
      output.textContent = `${value.title}: ${value.currency} ${value.ratio} ${value.memo ?? "(no memo)"}`;
    };
    // Scalar handles expose `value`: assigning shows it at once and commits once it settles.
    const control = (element, handle, read) => {
      const commit = () => { handle.value = read(element.value); };
      element.addEventListener("change", commit);
      return { sync: () => { element.value = String(handle.value); }, destroy: () => element.removeEventListener("change", commit) };
    };
    const controls = [control(ratio, doc.fields.ratio, Number), control(currency, doc.fields.currency, String)];
    const stop = doc.subscribe(() => { render(); controls.forEach((c) => c.sync()); });
    render();
    controls.forEach((c) => c.sync());
    return {
      unmount() {
        stop();
        controls.forEach((c) => c.destroy());
        target.replaceChildren();
      },
    };
  },
};
