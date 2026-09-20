# Collector follow-up evidence

The persistent multi-directory trash collector is new authorized feature work.
Its acceptance is functional testing and review, not a restarted timed contest.

The completed judging belongs to immutable baseline commit `176da86`. The exact
committed state is archived at `progress/baseline-176da86.json`: six product
pieces won, calibration passed, and all twelve verdicts remain intact. Reference
copies, blind packets, rejected votes, source hashes and measured release hashes
remain historical evidence for that baseline.

`progress/state.json` now describes the collector follow-up. Its current visual,
whole-interface and performance counts do not inherit the baseline's wins. No
new visual judging or benchmark victory is claimed. Historical round counts are
retained with `scope: baseline-176da86` and
`counts_for_current_release: false`.

The collector is complete and functionally verified: **72 of 72 checks pass**
(41 Rust unit, 17 existing terminal, and 14 collector terminal checks).
`validation/collector.json` records the logs, source hashes, and four headless
screenshots. The referee verified the release and all 11 source hashes, inspected
the four saved screenshots, and matched their capture manifests to release
`6f530a0ea119498849c89b089e3c3e143ccc5308adf0005199f856e02d67c3a9`.
Collection persists across navigation and rescans within a session, not after
exit. This release has not been visually rejudged or benchmarked.

 No deadline applies to this follow-up, and the old
source and deadline watchers remain stopped. The static progress server remains
live and displays the baseline and current work with separate labels.
