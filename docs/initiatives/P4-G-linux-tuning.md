# G: Linux tuning (optional)

Phase 4 (Performance) · Depends on: D · Size: S–M · Requires: a Linux machine (spare PC, mini PC, cloud VM or bare-metal instance)

## Vision

macOS hides much of what real HFT servers control: you can't pin threads to cores, isolate cores from the OS, or steer interrupts. This initiative runs the threaded pipeline (D) on a tuned Linux box and measures how much the OS was costing. It's the closest this project gets to a production trading server.

**Success looks like:** a documented, repeatable Linux setup, and a comparison table of the same benchmarks across:
- macOS
- untuned Linux
- tuned Linux

The tail latency (p99.9) difference is visible and explained.

**You'll learn:**
- how the OS scheduler interferes with latency
- CPU isolation
- interrupts and IRQ affinity
- CPU frequency scaling and C-states
- NUMA basics
- `perf`
- how production trading servers are configured

## Key concepts

- **`isolcpus` / `nohz_full`:** kernel boot options that keep the scheduler and timer ticks off chosen cores, so your threads have them to themselves.
- **Core pinning:** bind a thread to one core (`sched_setaffinity`, the `core_affinity` crate, `taskset`).
- **IRQ affinity:** route network and disk interrupts away from your isolated cores.
- **CPU governor / C-states:** power saving lowers clock speed and puts idle cores to sleep. Waking up costs microseconds. Use the `performance` governor and limit deep C-states.
- **Cloud caveat:** VMs share hardware with "noisy neighbors"; bare metal is far more stable. Results on a cheap VM will still be noisy.

## Design sketch

- A `docs/linux-setup.md` runbook: OS choice (e.g. Ubuntu LTS), kernel boot parameters, governor, IRQ steering, verification commands.
- An optional setup script that applies and verifies the tuning (idempotent, with clear warnings because it changes system settings).
- A benchmark matrix: the same commit, the same seed and A's measurement mode, across the three environments.
- Thread-to-core mapping configuration in the app (which pipeline thread goes on which core).

## Task breakdown

| # | Task | Owner | Depends on |
|---|---|---|---|
| G.1 | Guide: Linux scheduler, isolation, IRQs, power management; reading list | [Claude] | none |
| G.2 | Choose and get the machine (spare PC, mini PC, or cloud bare metal); decide on budget | [You] | none |
| G.3 | Install Rust and the project; run the untuned benchmarks | [You] | G.2, D |
| G.4 | **Tuning**: boot parameters, governor, IRQ affinity, disabling unneeded services, following the runbook as it's written | [You] | G.3, G.1 |
| G.5 | Runbook `docs/linux-setup.md`, written alongside G.4 | [Together] | G.4 |
| G.6 | Thread-to-core mapping config in the app; real pinning on Linux | [Claude] | D |
| G.7 | Use `perf stat` / `perf record` to check for context switches, cache misses and migrations on the hot threads | [Together] | G.4, G.6 |
| G.8 | Benchmark matrix: macOS vs untuned Linux vs tuned Linux | [You] | G.4, G.6 |
| G.9 | Write-up: which tweak gave the most, and why | [You] | G.8 |

## Pitfalls

- Changing many settings at once; you won't know what helped. Change one thing, then measure.
- Isolating all cores and leaving none for the OS.
- Benchmarking over SSH while other things run.
- Cloud VMs giving very noisy results. Note this in the write-up rather than over-interpreting.

## Done when

- [ ] The runbook reproduces the tuned setup from scratch.
- [ ] The benchmark matrix is committed, with p50/p99/p99.9 for each environment.
- [ ] `perf` shows no context switches on the pinned hot threads in steady state.
