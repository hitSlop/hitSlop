# Paycheck Flow design

One job: show where a month of money goes, in a picture you can read at a glance.

The object is a clean ledger card: income nodes on the left in green, a single dark
hub, and spending nodes on the right in six named colours with ribbons between them.
Layer Cake measures the box and stacks two layers: SVG for ribbons and nodes, HTML
for crisp name-and-amount labels. d3-sankey places everything. Hovering a ribbon or
node dims the rest and says its amount and share in words. What is left over is a
gold node; a shortfall enters from the left as a red "Over budget" source so ribbons
always balance, and the summary says "Over budget by …" in words.

Saved: month, currency, and each income and spending row (name, amount, colour).
Leftover and shares are derived, never stored. Amounts commit on change; zero rows
are hidden from the chart but kept in the sheet. Export is the same chart at a fixed
size with no hover; the icon is a flat sankey.
