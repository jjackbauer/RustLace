# Glossary

| Term | Meaning in RustLace | Source |
|---|---|---|
| **Natural (`Nat`)** | Arbitrary-precision non-negative integer. Base-2^64 limbs, little-endian, no most-significant zero limb; zero is the empty array. | `lovelace-nat/src/lib.rs:1` |
| **Integer (`Int`)** | Sign plus `Nat` magnitude. Sign is −1, 0 or +1; no negative zero. Division truncates toward zero; `/0` is `None`. | `lovelace-int/src/lib.rs:1` |
| **Real** | Arbitrary-precision decimal: `int * 10^exp`, optionally with period metadata. Exact periodic decimals are stored with the repeating block once. | `lovelace-real/src/lib.rs:1` |
| **Period notation** | `preperiod(period)`, e.g. `0.(3)` for 1/3 and `0.1(6)` for 1/6. A truncated irrational prints its digits with no ellipsis in the corpus. | `CONTRACT.md` (rendering rules) |
| **LReal64 / LReal128** | The fixed-width real forms the reference implementation uses for its fast paths. In Rust they appear as the `<= FAST_PATH_MAX_DIGITS` (37) digit gate that selects a fixed-width kernel instead of the arbitrary-precision one. | `lovelace-lang/src/complex.rs`, `lovelace-desktop/src/precision_default.rs:56` |
| **Complex** | A `(re, im)` pair of `Real`s, printed with the reference's `i` spelling; `complex` is also the domain constant name. | `lovelace-lang/src/complex.rs` |
| **Value** | The language's runtime value: `Natural`, `Integer`, `Real`, `Boolean`, `Text`, `Vector`, `Array`, `Function`, `Void`, `Complex`, `Symbolic`, `Record`, `Domain`, `Enum`. | `lovelace-lang/src/lib.rs:201`+ |
| **Kind** | The type name a value reports, used in typed rendering `<display> (<Kind>)`. | `Value::kind_name`, `lib.rs:240` |
| **Display precision** | The fractional-digit cap applied when a `Real` is *rendered*, separate from the computation budget. | `Engine::display_precision`, `lib.rs:1031` |
| **Precision** | The computation digit budget used by division, `sqrt`, `pi`, etc. Desktop default 30, engine default 1000. | `lib.rs:836`, `engine.rs:54` |
| **Modus / ModusHost** | The plugin seam: registration, descriptors, arity enforcement, typed payload channel, field kernels, load/lifecycle refusals. | `lovelace-lang/src/modus.rs:1` |
| **ModusPlugin** | The trait a plugin implements: `name()`, `register(host)`, optional assumption context. | `modus.rs:927` |
| **MathIR** | The intermediate representation the engine publishes for the runner and inspection (`--math-ir` version 2). | `lovelace-lang/src/mathir.rs` |
| **IncrementalRunner / DocumentCache** | The desktop's per-document cache of statement results, keyed on normalized source, precision, dependency values and function revisions. | `lovelace-desktop/src/incremental.rs:1` |
| **Statement prefix** | The committed prefix of a run: the statements that completed, in order. Panes and cache are consistent with exactly this prefix. | `execution.rs:55` |
| **Statement boundary** | The point between two top-level statements where cancellation is checked and committed results end. | `lib.rs:1595` |
| **Run / DocumentId / Revision** | A run belongs to one document and one source revision; events from a stale revision are ignored by the sink. | `execution.rs:41`–`51` |
| **WorkspaceSnapshot** | The atomic unit of pane state: result, structured projection, inspection, variables, functions, plot, output, precision, revision, committed/total. | `execution.rs:61` |
| **Recovery record** | `LOVELACE-RECOVERY` journal file (`.lvrec`) holding one document's unsaved bytes and caret/selection. | `lovelace-desktop/src/recovery.rs:1` |
| **Atomic save** | Temp file in the target's directory, flushed, synced, renamed over the target. | `document_io.rs:438` |
| **Corpus / parity gate** | `parity/corpus/*.json` – documented example, script, kind and expected text; `lovelace-corpus` runs them all. | `lovelace-corpus/src/main.rs:1` |
| **Oracle (C# oracle)** | The reference implementation whose published behaviour the corpus and the port pin. Sources live in the LovelaceSharp repository; the fixtures carried here are described in `docs/rustlace-notes.md`. | `docs/rustlace-notes.md` |
| **C# Studio** | The web IDE in the reference repository, the behavioural model the desktop preserves where CON-003 requires it. | `docs/parity-with-studio.md` |
| **PAL** | The C# `Lovelace.Suite` layer the Rust crates were ported from; its names survive in comments when they explain a behaviour. | comments throughout `lovelace-lang` |
| **TOFU** | "Text or fallback": a glyph the installed font lacks falls back to its ASCII spelling, never a tofu box. | `--shortcuts` output, `glyph_audit.rs` |
