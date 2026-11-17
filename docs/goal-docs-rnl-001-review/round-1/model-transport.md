# Model transport — review Finding 1 at HEAD `8ec1aaf`, with the concurrent working-tree state

## Tree state and time ordering — read this first

The reproduction is against the **committed tree the review targeted**, and the working tree changed
under this observer while the note was being written. The two artifact states are:

| When (2026-11-17, local) | `systems_family.rs` | runner binary | What it is |
|---|---|---|---|
| 11:48:31 source / 12:16:51 build | SHA256 `30108CEC0E384663BFC1C20B57B2060E9492CA25742D2B7016ACBA7B58078FCA` | SHA256 `1CCDC78951CF4E90D38E7F80EB07C1BA663D7E503052C3BA956D65CC0BA4AB4B` | working tree clean at HEAD `8ec1aaf9194b3946be5774755397150514dad21f` (`8ec1aaf`); the state used for OBS-01..OBS-44 and sections 1–5 (all runs before 12:59) |
| 12:59:17–12:59:56 source / 12:59:30 build | SHA256 `0CEF27B47A87CDC1322CAB840FC151D491A4958E934A9F550A3A20FEB2C2A6C6` at 12:59:56 | SHA256 `EAD75119B138CE522623BD0D0C458355B6C168663EAC256F92481319C2998B15` at 12:59:30 | concurrent **uncommitted** change (+238 lines in this file, plus `lovelace-rational` files; `git status` below); used only for OBS-45..OBS-49 |

The source file was edited again 26 s **after** that second binary was built, so the post-change rows
describe the `EAD75119` binary, not proof about the latest working tree. This run made none of those
edits. The first binary is no longer on disk; its outputs below are as captured before 12:59.

`git status --porcelain` at 13:00:14: `M .gitignore`, `M docs/benchmarks-numerics.md`,
`M lovelace-lang/src/systems_family.rs`, `M lovelace-rational/src/lib.rs`,
`M lovelace-rational/tests/properties.rs`, `D  tools/__pycache__/check-architecture.cpython-312.pyc`,
`?? .github/CODEOWNERS`, `?? .tmp-rational-baseline.txt`, `?? docs/goal-docs-rustlace-m0/`,
`?? docs/goal-docs-rnl-001-review/`.

**Scope of this note:** only `sys_*` construction/re-analysis in `lovelace-lang/src/systems_family.rs`
and the runner. No quality judgement and no remedy is proposed. Every fact below is read from a
quoted line or executed; the one generalisation is labelled ❓.

**Verdict at committed HEAD `8ec1aaf`:** Finding 1 reproduces. `sys_station("cpu", 1/97, 1)`
succeeds, but both end-to-end scripts fail on their **second** statement — `sys_open(1, [s])` and
`sys_closed(2, 1/97, [s])` — with `InvalidArgument`/`TypeMismatch`, because
`station_record`/`network_record` carry only `Real` projections computed under the 30-place
`DEFAULT_PRECISION` and `Ctx::number` refuses an inexact `Real` on re-parse. In those two scripts
`sys_analyze(m)`/`sys_mva(m)` are never reached (`m` is never assigned); with the network parameter
itself set to `1/97`, `sys_analyze`/`sys_mva` fail on the same gate.

**Verdict at the concurrent working tree (binary `EAD75119`, 12:59:30):** both end-to-end scripts
exit 0 and the records carry `exact_values` denominator 97 (OBS-45..OBS-49). The source/binary
mismatch above is recorded; this is an observation of that binary, not an assessment of the change.

## OBS entries

OBS-01..OBS-44 cite the **committed-HEAD** file (SHA256 `30108CEC…`, the state read at 11:48:31 and
used for every run before 12:59); OBS-45..OBS-49 are the concurrent post-change observations of
binary `EAD75119`.

| ID | Source (file:line) | Fact | Confidence |
|---|---|---|---|
| OBS-01 | `lovelace-lang/src/systems_family.rs:23` | The construction/transport precision is a constant: `pub const DEFAULT_PRECISION: usize = 30;` | Certain — read |
| OBS-02 | `lovelace-lang/src/systems_family.rs:73,85-86` | `sys_station` reads argument 2 with `Ctx::number` — `let service_time = ctx.number(&args[1], 1)?;` — and builds `Station::new(id, discipline, service_time, visits)`. | Certain — read |
| OBS-03 | `lovelace-lang/src/systems_family.rs:87` | `sys_station` serializes with the fixed constant, not engine precision: `Ok(station_record(&station, DEFAULT_PRECISION))`. | Certain — read + run (OBS-28) |
| OBS-04 | `lovelace-lang/src/systems_family.rs:99,115` | `sys_open` and `sys_closed` also serialize with the constant: `Ok(network_record(&network, DEFAULT_PRECISION))`. | Certain — read |
| OBS-05 | `lovelace-lang/src/systems_family.rs:450-455` | The projection helper is `Value::Real(Real::from_rational(q, precision).value)`; the conversion's `lossless` flag is discarded. | Certain — read |
| OBS-06 | `lovelace-lang/src/systems_family.rs:486-501` | `station_record`'s field set is `id`, `discipline`, `service_time`, `visits`, `service_demand` (+ optional `servers`): `("service_time", real(&station.service_time, precision))`, `("visits", real(&station.visits, precision))`, `("service_demand", real(&station.service_demand(), precision))`; the record has no `exact_values` field. | Certain — read + run (OBS-18) |
| OBS-07 | `lovelace-lang/src/systems_family.rs:503-528` | `network_record`'s field set is `name`, `workload`, `stations`, `precision`, then `arrival_rate` (open) at line 521 `("arrival_rate", real(&open.arrival_rate, precision))` or `population`+`think_time` (closed) at line 524 `("think_time", real(&closed.think_time, precision))`; the record has no `exact_values` field. | Certain — read + run (OBS-27) |
| OBS-08 | `lovelace-lang/src/systems_family.rs:554-569` | Analysis results do carry the sidecar: `station_metrics_fields` ends with `fields.push(("exact_values", exact_values(exact)));` (line 569). | Certain — read |
| OBS-09 | `lovelace-systems/src/model.rs:84-93,97-109`; `lovelace-systems/src/lib.rs:85` | The typed model holds the exact value: `pub service_time: Q`, and `Station::new` stores the argument unchanged (`service_time,`); `pub use lovelace_rational::Rat as Q;`. | Certain — read |
| OBS-10 | `lovelace-real/src/lib.rs:274-291` | `Real::from_rational` returns `Conversion::exact(real)` when `real.is_exact()`, otherwise `Conversion::truncated(real, max_digits.max(1))`. | Certain — read |
| OBS-11 | `lovelace-real/src/lib.rs:299-308` | `Real::to_rat` returns `Conversion::exact(value)` when `self.is_exact()`, otherwise `Conversion::from_truncated_input(value, self.frac_digits())`. | Certain — read |
| OBS-12 | `lovelace-rational/src/provenance.rs:143-159,171-173` | `Conversion::truncated` and `Conversion::from_truncated_input` both set `lossless: false`; `is_lossless()` returns that field. | Certain — read |
| OBS-13 | `lovelace-lang/src/systems_family.rs:225-239` | `Ctx::number` accepts a `Value::Real` only if `conversion.is_lossless()` after `let conversion = real.to_rat();`; otherwise it returns ``self.error(index, "an exact number (an integer, fraction or exact decimal)", format!("a precision-limited Real `{real}`"))``. | Certain — read + run (OBS-20, OBS-21) |
| OBS-14 | `lovelace-lang/src/systems_family.rs:202-211` | The refusal carries `"InvalidArgument"` / `"TypeMismatch"` and the template `"{}(): argument {} expects {expected}; got {arrived}."` (`index + 1`). | Certain — read + run |
| OBS-15 | `lovelace-lang/src/systems_family.rs:327-328` | `parse_station` re-reads the two numeric fields with the gate: `let service_time = ctx.number(field(ctx, fields, "service_time", index)?, index)?;` and `let visits = ctx.number(field(ctx, fields, "visits", index)?, index)?;`. | Certain — read |
| OBS-16 | `lovelace-lang/src/systems_family.rs:361-369` | `parse_network` parses `stations` first (line 361 `ctx.stations(...)`), then the workload: line 364 `arrival_rate: ctx.number(...)`, line 368 `think_time: ctx.number(...)`. | Certain — read |
| OBS-17 | `target\debug\lovelace-run.exe --stdin --text` (command A) | `sys_station("cpu", 1/97, 1)` exits 0 and prints `service_time: 0.010309278350515463917525773195` — 30 fractional digits, no period parentheses. | Certain — run |
| OBS-18 | command B | `s.exact_values` is refused: `record 'Station' has no member 'exact_values'. Available members: id, discipline, service_time, visits, service_demand, servers.` | Certain — run |
| OBS-19 | commands C and `inspect` | `inspect(1/97)` reports `exact: True`; `inspect(s.service_time)` reports `exact: False`; `s.service_time == 1/97` is `False`. (`inspect` maps a Real to `r.is_exact()`, `lovelace-lang/src/builtins.rs:2029-2032`.) | Certain — read + run |
| OBS-20 | command D, `--json` envelope | The open script fails on line 2 (`m = sys_open(1, [s])`): `ok=false`, `code=InvalidArgument`, `category=TypeMismatch`, `diagnostics[0].line=2`, `.column=5`, `.position=37`, exit 1; `partialVariables` contains only `s`. | Certain — run |
| OBS-21 | command E, `--json` envelope | The closed script fails on line 2 (`m = sys_closed(2, 1/97, [s])`): `ok=false`, `code=InvalidArgument`, `category=TypeMismatch`, `diagnostics[0].line=2`, `.column=5`, `.position=37`, exit 1; `m` is not in `partialVariables`. | Certain — run |
| OBS-22 | command F | `sys_analyze` on a network whose `arrival_rate` is `1/97` fails at argument 1: ``sys_analyze(): argument 1 expects an exact number (an integer, fraction or exact decimal); got a precision-limited Real `0.010309278350515463917525773195`.`` The network record itself was built. | Certain — run |
| OBS-23 | command G | `sys_mva` on a network whose `think_time` is `1/97` fails at argument 1 with the same `precision-limited Real` refusal. The network record itself was built. | Certain — run |
| OBS-24 | command H | Control: `m = sys_open(1, [sys_station("cpu", 1/10, 1)])` followed by `sys_analyze(m)` exits 0 (`ok=true`). | Certain — run |
| OBS-25 | command I | `1/7` (period 6) survives `sys_station → sys_open`: `s.service_time` prints `0.(142857)` and `sys_open(1, [s]).workload` is `open`. | Certain — run |
| OBS-26 | command J | `1/61` (period 60) fails `sys_station → sys_open` with the same `precision-limited Real` refusal at `sys_open(): argument 2`. | Certain — run |
| OBS-27 | commands K and L | The built records lose the value: `m.think_time == 1/97` is `False`, `m.arrival_rate == 1/97` is `False`; the wire form is `{"exact":false,"kind":"Real","value":"0.010309278350515463917525773195"}`. | Certain — run |
| OBS-28 | command M | `setprecision(1000); s = sys_station("cpu", 1/97, 1)` still gives `inspect(s.service_time).exact == False`, still prints 30 digits, and `sys_open(1, [s])` still fails. | Certain — run |
| OBS-29 | command N | A long-period `visits` (`sys_station("cpu", 1/10, 1/97)`) also fails `sys_open(1, [s])` with the same refusal at argument 2. | Certain — run |
| OBS-30 | command O | `1/(10^29-1)` (period 29) survives `sys_station → sys_open`; `s.service_time` prints `0.(00000000000000000000000000001)` (29 digits). | Certain — run |
| OBS-31 | command P | `1/(10^30-1)` and `1/211` (each period 30) are exact as literals (`0.(...)`, 30 digits), but their station `service_time` projections print 30 digits with no period and fail `sys_open`. | Certain — run |
| OBS-32 | command Q | `1/2^100` (terminating, 100 fractional digits) survives `sys_station → sys_open`; the field prints all 100 digits. | Certain — run |
| OBS-33 | `lovelace-real/src/lib.rs:651-667` | `from_fraction` answers a terminating expansion before the digit loop via `terminate_exactly`, with `inexact: false`, at any budget. | Certain — read |
| OBS-34 | `lovelace-real/src/lib.rs:634-637,685-696,703-708` | The digit loop tests `if position >= max_digits { break }` (line 689) **before** the repeated-remainder test `if let Some((_, idx)) = seen.iter().find(...)` (line 692), which sets `period_len = (position - *idx)`; while the position budget applies, a repeat first seen at decimal position ≥ 30 is therefore cut, including a period that closes exactly at the cap. A zero run that alone reaches `max_digits` sets `freed = true` (lines 703-708); from then on the break tests `digits.len() >= max_digits` (stored digits) instead of the decimal position. | Certain — read |
| OBS-35 | `lovelace-run/src/main.rs:409,435-436` | Text mode prints `= {typed}` on success; on failure it prints `Error [{code}/{category}]: {message}` to stderr and calls `std::process::exit(1)`. | Certain — read + run |
| OBS-36 | `lovelace-lang/src/lib.rs:833`; `lovelace-lang/src/value.rs:283-291,342` | `Engine::new` sets `precision: 1000`; natural division reaches `real_ratio_nat`, which calls `Real::from_fraction(num, den, precision)` with that engine precision. | Certain — read |
| OBS-37 | `lovelace-lang/src/systems_family.rs:845-1077`; grep | At HEAD the test module contains ten tests and none is named `long_period_service_time_survives_station_and_open` or `long_period_think_time_survives_closed_and_mva`; no HEAD source line in the module contains `1/97`. | Certain — read (+ `grep long_period` found only unrelated `lovelace-real` tests) |
| OBS-38 | command R | `1/(2^10*7)` (non-repeating prefix 10 + period 6; first repeated remainder at decimal position 16) survives `sys_station → sys_open`; the field prints `0.0001395089(285714)`. | Certain — run |
| OBS-39 | command S | `1/(2^25*7)` (prefix 25 + period 6; first repeat at position 31) and `1/(2^40*7)` (prefix 40 + period 6; first repeat at position 46) fail `sys_station → sys_open` with the `precision-limited Real` refusal, although the literals are exact and display a period. | Certain — run |
| OBS-40 | command T | `1/(10^40*7)` (40-digit leading-zero run, then period 6) survives `sys_station → sys_open`; the field prints `0.0000000000000000000000000000000000000000(142857)`. | Certain — run |
| OBS-41 | command U | `1/(10^40*211)` (40-digit zero run, then period 30 whose first two digits are zeros) survives `sys_station → sys_open`; the field prints the full 30-digit period `(004739336492890995260663507109)`. | Certain — run |
| OBS-42 | command V | `1/(10^40*(10^60-1))` (40-digit zero run, then a 60-digit period of 59 zeros and one `1`) survives `sys_station → sys_open`; the field prints the full 60-digit period. | Certain — run |
| OBS-43 | command W | `1/(10^40*(10^96-1))` (40-digit zero run, then a 96-digit period of 95 zeros and one `1`) survives `sys_station → sys_open`; the field prints the full 96-digit period. | Certain — run |
| OBS-44 | command X | `1/(10^40*97)` (40-digit zero run, then period 96 with 95 stored digits) fails `sys_station → sys_open`; the field is the 30-stored-digit truncation `0.00000000000000000000000000000000000000000103092783505154639175257731958`. | Certain — run |
| OBS-45 | command Y (working-tree binary `EAD75119`, 12:59:30) | The open script (`s = …; m = sys_open(1, [s]); sys_analyze(m)`) exits 0 (`Analysis(...)`); `sys_analyze(m).stations[0].exact_values.service_time.denominator` is 97. | Certain — run (post-change) |
| OBS-46 | command Z (same binary) | The closed script (`s = …; m = sys_closed(2, 1/97, [s]); sys_mva(m)`) exits 0; the aggregate `exact_values.think_time.denominator` is 97 and `stations[0].exact_values.service_time.denominator` is 97. | Certain — run (post-change) |
| OBS-47 | command AA (same binary) | `m = sys_open(1/97, [sys_station("cpu", 1/10, 1)]); sys_analyze(m).exact_values.throughput.denominator` prints `97`, and the closed `think_time` variant prints `97`. | Certain — run (post-change) |
| OBS-48 | command AA (same binary) | The four fractions that failed at HEAD through `sys_station → sys_open` — `1/61`, `1/211`, `1/(2^40*7)`, `1/(10^40*97)` — now exit 0 with station `exact_values.service_time.denominator` 61, 211, 7696581394432 and 970000000000000000000000000000000000000000; the control `1/2^100` gives denominator 1267650600228229401496703205376. | Certain — run (post-change) |
| OBS-49 | mtimes and hashes at 13:00:14 | The working source was edited again after the post-change binary was built: `systems_family.rs` mtime 12:59:56 (SHA256 `0CEF27B4…`) is newer than `lovelace-run.exe` mtime 12:59:30 (SHA256 `EAD75119…`); OBS-45..OBS-48 therefore describe that binary, not a proven build of the latest source. | Certain — observed |

## Reproduction

**Method note.** The runner uses `Engine::new()` (`lovelace-run/src/main.rs:361`), and `--eval`,
`--stdin` and `--file` all end in the same `engine.eval(source)` call
(`lovelace-run/src/main.rs:346-353,450-453`). This PowerShell 5.1 host strips embedded double quotes
from native arguments, so
`--eval 'sys_station("cpu", 1/97, 1)'` arrives as `sys_station(cpu, 1/97, 1)` and fails with
`Undefined variable 'cpu'.`; scripts below were therefore supplied on stdin. Working directory
`F:\dev\RustLace`. The binary is the HEAD-state one in the timeline.

**Command labels used in the OBS table.** A `sys_station("cpu", 1/97, 1)`; B `s.exact_values`;
C `inspect(1/97)` / `inspect(s.service_time)` / `s.service_time == 1/97`; D open script below;
E closed script below; F `sys_analyze` on `sys_open(1/97, [1/10 station])`; G `sys_mva` on
`sys_closed(2, 1/97, [1/10 station])`; H open control `sys_open(1, [1/10 station])`; I `1/7`;
J `1/61`; K closed `think_time` projection; L open `arrival_rate` projection;
M `setprecision(1000)`; N `visits = 1/97`; O `1/(10^29-1)`; P `1/(10^30-1)` and `1/211`;
Q `1/2^100`; R `1/(2^10*7)`; S `1/(2^25*7)` and `1/(2^40*7)`; T `1/(10^40*7)`;
U `1/(10^40*211)`; V `1/(10^40*(10^60-1))`; W `1/(10^40*(10^96-1))`; X `1/(10^40*97)`;
Y open script on binary `EAD75119`; Z closed script on `EAD75119`; AA `1/97` network parameters and
the boundary fractions on `EAD75119`. Each D–X command was executed in the HEAD state; Y–AA in the
post-change state.

### 1. End-to-end script `sys_station → sys_open → sys_analyze`

```powershell
$exe = ".\target\debug\lovelace-run.exe"
$open = @'
s = sys_station("cpu", 1/97, 1)
m = sys_open(1, [s])
sys_analyze(m)
'@
$open | & $exe --stdin --text
```

Observed (stderr; exit code 1):

```
Error [InvalidArgument/TypeMismatch]: sys_open(): argument 2 expects an exact number (an integer, fraction or exact decimal); got a precision-limited Real `0.010309278350515463917525773195`.
```

The failing statement is line 2, `m = sys_open(1, [s])`; `sys_analyze(m)` (line 3) is not reached.
Selected fields from the same source with `--stdin --json` (the full envelope also carries the
builtin `functions` array, omitted here):

```
ok=false  code=InvalidArgument  category=TypeMismatch
diagnostics[0]: line=2  column=5  position=37
diagnostics[0].message:
  sys_open(): argument 2 expects an exact number (an integer, fraction or exact decimal);
  got a precision-limited Real `0.010309278350515463917525773195`.
partialVariables: [ "s" ]        (no "m")
timings: [ {position:1, resultKind:"Record"}, {position:33, resultKind:"Void"} ]
```

### 2. End-to-end script `sys_station → sys_closed → sys_mva`

```powershell
$closed = @'
s = sys_station("cpu", 1/97, 1)
m = sys_closed(2, 1/97, [s])
sys_mva(m)
'@
$closed | & $exe --stdin --text
```

Observed (stderr; exit code 1):

```
Error [InvalidArgument/TypeMismatch]: sys_closed(): argument 3 expects an exact number (an integer, fraction or exact decimal); got a precision-limited Real `0.010309278350515463917525773195`.
```

The failing statement is line 2, `m = sys_closed(2, 1/97, [s])`; `sys_mva(m)` (line 3) is not
reached. Selected fields from `--stdin --json`:

```
ok=false  code=InvalidArgument  category=TypeMismatch
diagnostics[0]: line=2  column=5  position=37
diagnostics[0].message:
  sys_closed(): argument 3 expects an exact number (an integer, fraction or exact decimal);
  got a precision-limited Real `0.010309278350515463917525773195`.
partialVariables: [ "s" ]        (no "m")
```

The error points at argument 3 (the station vector); `sys_closed` itself succeeds when the same
`1/97` is the `think_time` and the station is exact (see below), so the refusal here comes from `s`
inside `[s]`.

### 3. `sys_analyze` and `sys_mva` on a long-period network parameter

```powershell
'm = sys_open(1/97, [sys_station("cpu", 1/10, 1)]); sys_analyze(m)'   | & $exe --stdin --text
'm = sys_closed(2, 1/97, [sys_station("cpu", 1/10, 1)]); sys_mva(m)' | & $exe --stdin --text
```

Observed (each exit code 1):

```
Error [InvalidArgument/TypeMismatch]: sys_analyze(): argument 1 expects an exact number (an integer, fraction or exact decimal); got a precision-limited Real `0.010309278350515463917525773195`.
Error [InvalidArgument/TypeMismatch]: sys_mva(): argument 1 expects an exact number (an integer, fraction or exact decimal); got a precision-limited Real `0.010309278350515463917525773195`.
```

Here the network record was built (the failing re-parse is of `arrival_rate` at
`systems_family.rs:364` and `think_time` at `systems_family.rs:368`), which isolates
`network_record`'s projection from the station projection.

### 4. `sys_station` itself, and the `service_time` field

```powershell
'sys_station("cpu", 1/97, 1)' | & $exe --stdin --text
```

Observed (exit code 0):

```
= Station(id: cpu, discipline: fcfs, service_time: 0.010309278350515463917525773195, visits: 1, service_demand: 0.010309278350515463917525773195, servers: 1) (Station)
  _ = Station(id: cpu, discipline: fcfs, service_time: 0.010309278350515463917525773195, visits: 1, service_demand: 0.010309278350515463917525773195, servers: 1)
```

* `sys_station` succeeds (exit 0).
* `service_time` is a scalar `Value::Real`; it has no member `exact_values`. The record has no
  `exact_values` member at all:

```powershell
's = sys_station("cpu", 1/97, 1); s.exact_values' | & $exe --stdin --text
```

```
Error [InvalidOperation/DomainError]: record 'Station' has no member 'exact_values'. Available members: id, discipline, service_time, visits, service_demand, servers.
```

* Structured/wire form of the field:

```powershell
$o = ('s = sys_station("cpu", 1/97, 1); s.service_time' | & $exe --stdin --json) | ConvertFrom-Json
$o.result.structured | ConvertTo-Json -Compress
```

```
{"exact":false,"kind":"Real","value":"0.010309278350515463917525773195"}
```

* For comparison, the literal is exact (period 96 digits), 30-digit projection is not:

```powershell
'inspect(1/97)' | & $exe --stdin --text
's = sys_station("cpu", 1/97, 1); inspect(s.service_time)' | & $exe --stdin --text
's = sys_station("cpu", 1/97, 1); s.service_time == 1/97' | & $exe --stdin --text
```

```
= Inspection(type: Real, domain: , exact: True, ...) (Inspection)
= Inspection(type: Real, domain: , exact: False, ...) (Inspection)
= False (Boolean)
```

(The two `Inspection` lines are abridged after the `exact` field; the field itself is verbatim.)
`1/97` itself crosses as `{"denominator":"97","exact":true,"kind":"Real","numerator":"1","value":"0.(...96 digits...)"}`
(the literal is accepted by `sys_station`, wire value abridged after `0.(`); the field the record
returns is `exact: false`.

### 5. Boundary at HEAD: which fractions round-trip today

Each row was executed as `sys_station("cpu", <f>, 1)` followed by `sys_open(1, [s])` (pattern
below); literal `exact` was read with `inspect(<f>).exact` and is `True` for every row.

```powershell
$f = '1/(10^40*(10^96-1))'
"s = sys_station(`"cpu`", $f, 1); [s.service_time, sys_open(1, [s]).workload]" | & $exe --stdin --text
```

| fraction | expansion | station → `sys_open` |
|---|---|---|
| `1/7` | period 6, first repeat at position 6 | **passes**, field `0.(142857)` |
| `1/(2^10*7)` | 10-digit prefix, then period 6; first repeat at position 16 | **passes**, field `0.0001395089(285714)` |
| `1/(2^25*7)` | 25-digit prefix, then period 6; first repeat at position 31 | fails, field `0.000000004257474626813616071428` (30 digits) |
| `1/(2^40*7)` | 40-digit prefix, then period 6; first repeat at position 46 | fails, field `0.000000000000129927814538989748` (30 digits) |
| `1/(10^29-1)` | period 29, first repeat at position 29 | **passes**, field `0.(00000000000000000000000000001)` |
| `1/(10^30-1)` | period 30, repeat at position 30 (not detected) | fails, field `0.000000000000000000000000000001` (30 digits) |
| `1/211` | period 30, repeat at position 30 (not detected) | fails, field `0.004739336492890995260663507109` (30 digits) |
| `1/61` | period 60 | fails, field `0.01639344262295081967213114754` (30 digits) |
| `1/97` | period 96 | fails, field `0.010309278350515463917525773195` (30 digits) |
| `1/(10^40*7)` | 40-zero run, then period 6 (6 stored digits) | **passes**, field `0.0000000000000000000000000000000000000000(142857)` |
| `1/(10^40*211)` | 40-zero run, then period 30 (2 period-leading zeros, 28 stored) | **passes**, field `0.0000000000000000000000000000000000000000(004739336492890995260663507109)` |
| `1/(10^40*97)` | 40-zero run, then period 96 (95 stored digits) | fails, field `0.00000000000000000000000000000000000000000103092783505154639175257731958` (30 stored digits) |
| `1/(10^40*(10^60-1))` | 40-zero run, then period 60 (59 zeros + `1`; 1 stored digit) | **passes**, field prints the full 60-digit period |
| `1/(10^40*(10^96-1))` | 40-zero run, then period 96 (95 zeros + `1`; 1 stored digit) | **passes**, field prints the full 96-digit period |
| `1/2^100` | terminating, 100 fractional digits | **passes**, field prints all 100 digits |

`setprecision(1000)` does not change any of this (OBS-28), because the construction functions pass
`DEFAULT_PRECISION` (30), not `engine.precision()`.

**Answer to "can any long-period fraction path pass today": yes — but not the Finding 1 value.**
The outcome is decided by where the repeated remainder is first seen relative to the applicable
30-place budget, not by the period length alone:

* With no long leading-zero run, the decimal-position budget applies; the repeat must appear before
  position 30. That is why `1/97` (period 96) fails and `1/(2^40*7)` (period 6, but repeat at
  position 46) also fails while `1/(2^10*7)` (repeat at 16) passes.
* A leading-zero run that alone reaches 30 sets `freed` (`lovelace-real/src/lib.rs:703-708`), after
  which the budget counts **stored** (non-leading-zero) digits. So a long period made mostly of
  zeros passes: `1/(10^40*(10^96-1))` has a 96-digit period, 1 stored digit, and survives
  `sys_station → sys_open` with its period intact (OBS-43); `1/(10^40*211)` keeps a 30-digit period
  with 28 stored digits (OBS-41); `1/(10^40*97)` still fails because 95 stored digits exceed the
  30-digit stored budget (OBS-44).
* A long *terminating* decimal of arbitrary length passes because `from_fraction` answers it
  exactly before the budget loop (OBS-32, OBS-33).

So the concrete Finding 1 claim (`1/97` through `sys_station → sys_open` / `sys_closed → sys_mva`)
reproduces at HEAD, while the broader statement "long-period fractions cannot cross" is false even
at HEAD: a zero-heavy 96-digit-period fraction crosses today. ❓ This enumeration covers the paths
read here (`sys_station`/`sys_open`/`sys_closed` → `sys_analyze`/`sys_mva`/`sys_bottlenecks`/
`sys_sweep`, all of which call `parse_network`); it does not claim to cover record transport outside
the `sys_*` adapter. On the concurrent post-change binary, `1/97` and the four failing periodic rows
above pass (OBS-45..OBS-49); that is a different tree state, addressed separately.

## Concurrent post-change observation (uncommitted; not the HEAD verdict)

At 13:00 the binary `EAD75119` (built 12:59:30, after the uncommitted edits listed in the timeline)
was run on the same two scripts:

* Script 1 exits 0 and returns an `Analysis(...)`; the decisive substring is
  `stations: [StationMetrics(... exact_values: ExactValues(..., service_time: ExactRational(numerator: 1, denominator: 97), ...))]`.
* Script 2 exits 0; the aggregate carries
  `exact_values: ExactValues(..., think_time: ExactRational(numerator: 1, denominator: 97))` and the
  station carries `service_time` denominator 97.
* `m = sys_open(1/97, [sys_station("cpu", 1/10, 1)]); sys_analyze(m)` and
  `m = sys_closed(2, 1/97, [sys_station("cpu", 1/10, 1)]); sys_mva(m)` also exit 0 with `97` values.
* The four fractions that failed at HEAD through `sys_station → sys_open` (`1/61`, `1/211`,
  `1/(2^40*7)`, `1/(10^40*97)`) and the control `1/2^100` exit 0 with the sidecar denominators in
  OBS-48.

This is a behaviour observation of that binary; it is not the HEAD verdict, and OBS-49 records that
the source was edited again after that binary was built. No judgement on the change is made here.

## What I could not determine within SCOPE

1. **Binary-to-HEAD rebuild equality.** Neither binary was built by this run. The pre-change binary
   (`1CCDC789`, 12:16:51) post-dates every HEAD source by mtime, but I cannot prove bit-for-bit
   that it is a fresh build of `8ec1aaf`; its observable messages match the HEAD lines quoted. The
   post-change binary (`EAD75119`, 12:59:30) was built by the concurrent work and is older than the
   latest source edit (OBS-49).
2. **The reviewer's literal `--eval` invocation.** PowerShell 5.1 stripped the embedded quotes
   (observed as `Undefined variable 'cpu'.`), so I used `--stdin`. Both flags reach the same
   `engine.eval(source)` (`main.rs:346-353,450-453`), but I did not execute the reviewer's exact
   shell command line.
3. **The exact line 3 of the two scripts.** `sys_analyze(m)`/`sys_mva(m)` never execute because
   `m` is never assigned. I showed the same gate failing for `sys_analyze`/`sys_mva` when the
   network's own `arrival_rate`/`think_time` is `1/97` (section 3), but not through a successfully
   constructed `m` carrying a long-period station.
4. **Paths outside `lovelace-lang/src/systems_family.rs`.** I did not inspect whether any other
   surface (desktop, CLI envelopes, other builtins) constructs or transports `Station`/
   `QueueNetwork` records; the five paths above are the `sys_*` adapter paths in scope.
5. **Exhaustive fraction coverage.** The HEAD boundary matrix is executed at 15 fractions (periods
   6, 29, 30, 60 and 96, with and without long prefixes or leading-zero runs, plus a 100-digit
   terminating decimal). The pass/fail model in section 5 is read from the budget and `freed`
   control flow at OBS-34 and illustrated by those rows; I did not enumerate every denominator, so
   an expansion shape outside the matrix is not claimed. The post-change checks cover only the four
   previously failing periodic cases plus the `1/2^100` control (OBS-48).
6. **The final working-tree state.** The tree changed at least twice during this observation
   (source SHA256 `30108CEC…` → `859951E9…` → `0CEF27B4…`; binary `1CCDC789…` → `EAD75119…`). The
   HEAD reproduction is anchored by hashes and mtimes, but I cannot state what a fresh build of the
   latest working tree would do; that tree and its change are outside this note's HEAD verdict.

**Repository-state note:** no tracked file was edited by this run; the only file this run wrote is
this note, inside the untracked `docs/goal-docs-rnl-001-review/` directory. At 13:00:14
`git status --porcelain` listed the five modified tracked files from the concurrent work (see the
timeline), the deleted `tools/__pycache__/check-architecture.cpython-312.pyc`, and four untracked
paths; none was created or modified by this run.
