# Observed gdu incomplete summaries

Installed gdu 5.37.0 intermittently reported incomplete totals in the isolated
benchmark guest. No output error marker or nonzero exit accompanied the mismatch.

- `final-invalid-gdu.log`: broad fixture warm trial 3 returned 1,305,346,048 bytes;
  the independent metadata audit expected 1,310,715,904 file bytes.
- `ui-final-invalid-gdu.log`: the 18-file capture fixture had four incomplete gdu
  totals among fourteen measurements; for example 3,256,889,344 bytes versus
  4,473,241,600 independently audited file bytes.
- All tui-disk measurements in these runs matched independent totals and counts.
- The entire affected runs were rejected. Their favorable tui-disk timings do
  not count as benchmark wins. A later valid broad run recorded a cold loss in
  `final.json` and remains a loss.

The [upstream documentation](https://github.com/dundee/gdu) distinguishes the
default noninteractive analyzer (top-level totals only) from the full-tree
analyzer used for interactive, `--top`, or `--depth` modes. The current benchmark
uses the faster default noninteractive mode, while tui-disk builds a full tree.

A possible cause is visible in the version-pinned
[TopDirAnalyzer](https://github.com/dundee/gdu/blob/v5.37.0/pkg/analyze/parallel_top_dir.go)
and [custom WaitGroup](https://github.com/dundee/gdu/blob/v5.37.0/pkg/analyze/wait.go):
the latter unlocks when the work count reaches zero; later additions do not
relock that wait mutex. A later wait can therefore return before subsequently
scheduled work finishes. This is a source-based inference, not a reproduced
standalone causal proof. No gdu source or installed binary was modified.

The primary comparator now uses the full analyzer through official export mode
with `--summarize` and selected root attributes. The version-pinned
[export implementation](https://github.com/dundee/gdu/blob/v5.37.0/report/export.go)
constructs `analyze.CreateAnalyzer()`, scans fully, updates stats, and summarizes
only when serializing. This matches the application's complete-tree workload
while keeping output small. The original summary engine remains an additional
separate comparator (`--gdu-engine summary`), and its cold loss is preserved.
The trial count, byte checks, and winner rule remain unchanged. No result with
incorrect totals is accepted.
