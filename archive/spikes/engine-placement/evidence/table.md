Values are **matched WASM / native**. Latency columns are the median of five fresh-process p95s; memory/opening are medians. Full ranges and all window counts are in results.json.

| Rows, one window | Checkbox acceptance ms | Move acceptance ms | Host + content MiB | Open ms |
|---|---|---|---|---|
| 1,000 | 7.0 / 2.0 | 7.0 / 2.0 | 152.4 / 103.8 | 427.1 / 354.3 |
| 5,000 | 16.0 / 5.0 | 35.0 / 6.0 | 180.7 / 124.1 | 500.6 / 448.1 |
| 40,000 | 72.0 / 29.0 | 253.0 / 33.0 | 540.7 / 184.7 | 1099.6 / 1397.3 |
