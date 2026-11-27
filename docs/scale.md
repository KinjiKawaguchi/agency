# How federation scales

What one pass of a home node's background work costs as the number of team nodes and teams grows, measured by
the federation load test (agency#529). Every 15 seconds the node runs one pass: `refresh` reads the view of every
team on another node, then each peer's `about` and the org definition; then the push pass and the outbox run.

## Running the load test

```sh
scripts/load/federation.sh
```

It builds in release mode and runs `crates/agency-node/tests/federation_load.rs`, which is ignored in the usual
test run. Every node runs in one process on the loopback, with memory runtimes, and is reached over HTTP as in
production. The home node is the org's root; each other node joins the org and hosts its teams. Each team's view
holds 20 requests. A "silent" node accepts connections and never answers, so each read of a team on it waits for
the reader's 8-second timeout.

Times are the median of three passes after a first one. CPU is the whole process's, so it counts the team nodes
answering as well as the home node reading, at a resolution of 10 ms. "Views" is the size of the views read in
one pass. "Decision" is one decision sent from the home node to a team node and applied there. "Home data" is the
home node's data directory after the run.

## Numbers

Machine: AMD EPYC 9115 (16 cores, 32 threads), 1.5 TB of memory, Linux 6.8. Code at `f2b62f1` (2026-11-27).

| team nodes | teams each | silent teams | remote teams | refresh ms | push ms | outbox ms | CPU ms per pass | views KiB per pass | decision ms | home data KiB |
|---|---|---|---|---|---|---|---|---|---|---|
| 2 | 10 | 0 | 20 | 4 | 1 | 0 | 0 | 84 | 0 | 181 |
| 5 | 50 | 0 | 250 | 99 | 14 | 0 | 110 | 1058 | 1 | 2188 |
| 10 | 200 | 0 | 2000 | 2866 | 124 | 0 | 3210 | 8474 | 3 | 18810 |
| 2 | 10 | 5 | 25 | 40017 | 1 | 0 | 10 | 84 | 1 | 196 |

## What the numbers say

- **One team node that does not answer costs 8 seconds per team on it, every pass.** Five such teams make a pass
  take 40 seconds instead of 4 ms, and two are enough to exceed the 15-second loop. The push pass and the outbox
  run after `refresh` in the same loop, so the owner's notifications and every queued decision wait that long.
  This is the first limit, and it doesn't depend on load (agency#530, agency#531).
- **Reading every remote team every pass grows linearly.** At 2,000 remote teams, `refresh` takes about 2.9 s and
  reads about 8.3 MiB of views per pass, which is about 33 MiB a minute even when nothing changed (agency#532).
  It still fits in the 15-second loop on this machine.
- **The push pass grows with the teams too** (124 ms at 2,000), since it reads every replica. The outbox costs
  nothing when it's empty.
- **Deciding stays cheap**: a few milliseconds end to end on the loopback, at every size.
- The data directory grows with the replicas it keeps: about 9 KiB per remote team at this view size.

These are loopback numbers on a large machine. A real link adds its latency to every read: since the reads are one
after another, 2,000 teams over a 50 ms link would add about 100 s to a pass.
