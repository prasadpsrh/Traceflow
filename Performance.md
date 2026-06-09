# Traceflow Performance Baselines

> Measured on: [your machine specs]
> Date: [date]
> Rust version: 1.90.0
> Display: [resolution]

## Pipeline stage latencies (p50 / p95)

| Stage | p50 | p95 | Budget | Status |
|---|---|---|---|---|
| frame_grab | ? ms | ? ms | 15 ms | ? |
| fingerprint | ? ms | ? ms | 5 ms | ? |
| diff | ? ms | ? ms | 2 ms | ? |
| ocr (windows) | ? ms | ? ms | 250 ms | ? |
| ocr (ocrs) | ? ms | ? ms | 400 ms | ? |
| encode_png | ? ms | ? ms | 100 ms | ? |
| hash | ? ms | ? ms | 5 ms | ? |
| write_frame | ? ms | ? ms | 20 ms | ? |

## Adaptive throttling behavior

| CPU utilization | Poll rate adjustment |
|---|---|
| < 40% | Base rate (configured fps) |
| 40–60% | Hold current |
| > 60% | Reduce by 1.5×, max 4× slowdown |

## Memory usage

| Scenario | RSS |
|---|---|
| Idle (no session) | ? MB |
| Active capture (1080p) | ? MB |
| Active capture (4K) | ? MB |
| 100-step session loaded | ? MB |