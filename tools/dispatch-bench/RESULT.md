# WO-007 dispatch benchmark — measured output
# host: 2-core virtualised linux sandbox, ~1 GB RAM. NOT SATURN, NOT Windows.
# cargo 1.98.1 · 2026-09-22T13:57:38Z · run 4, from tools/dispatch-bench after the move into the repo

WO-007 dispatch benchmark — 100 modules x 64-sample block, release profile (opt-level 3, thin LTO, cg-units 1)
  fixture: 100 modules over 4 distinct implementors (util/gain-a, util/gain-b, util/gain-c, util/gain-d) — the vtable pointer varies at runtime, as it would in a real graph
host: 2 cores visible, linux — NOT the stage device; ratios transfer, absolutes do not

A. ONE DISPATCH PER MODULE PER BLOCK (what the executor does)
  A1  Box<dyn Module>  (trait object) median      1700 ns   p99      2209 ns   min      1643 ns
  A2  enum + match  (generated)      median      1275 ns   p99      1669 ns   min      1234 ns
  A3  monomorphised  (no dispatch)   median      1281 ns   p99      1673 ns   min      1242 ns

B. ONE DISPATCH PER MODULE PER SAMPLE (the case the work order warns about)
  B1  Box<dyn Module>  per sample    median     27972 ns   p99     44641 ns   min     26519 ns
  B2  enum + match  per sample       median      2109 ns   p99      2910 ns   min      2068 ns
  B3  monomorphised  per sample      median      2121 ns   p99      2799 ns   min      2062 ns

C. VERDICT AGAINST THE ACCEPTANCE TARGET (< 20 us per block of 100 modules)
  Box<dyn Module>, block dispatch      1.700 us/block     11.8x headroom vs the 20 us target
  enum + match, block dispatch         1.275 us/block     15.7x headroom vs the 20 us target
  monomorphised, block dispatch        1.281 us/block     15.6x headroom vs the 20 us target

D. PER-CALL COST (the number that decides Q1)
  block dispatch: dyn 17.0 ns/call · enum 12.8 ns/call · mono 12.8 ns/call  (dyn/enum = 1.33x)
  sample dispatch: dyn 4.37 ns/call · enum 0.33 ns/call · mono 0.33 ns/call  (dyn/enum = 13.27x)

  full per-sample graph cost: dyn 27.972 us/block vs enum 2.109 us/block (6400 calls)
