//! ARC-003: the mechanical source audit that keeps the native UI out of the engine.
//!
//! The rule is not "the UI happens not to call the engine today". It is a set of syntactic
//! invariants over every **production** file under `src/ui/` — comments and `#[cfg(test)]` modules
//! are stripped first, so a test-only evaluator cannot mask a production violation:
//!
//! 1. no evaluation call: no `eval(`, `.eval(`, `eval_full(`;
//! 2. no engine construction: no `Engine::new`, `RealEngine::new`, `FakeEngine::new`, and no
//!    `default_engine_factory`;
//! 3. no compiler invocation: no `compile(`;
//! 4. engine imports name data types only (`EngineState`, `EngineFunction`, `FunctionOrigin`, …),
//!    never `Evaluator` or a concrete engine;
//! 5. execution imports name message/state types only (`Diagnostic`, `DocumentId`,
//!    `SourceRevision`, `WorkspaceSnapshot`, …), never `ExecutionHost`, `EventSink` or
//!    `RunHandle`;
//! 6. every `crate::` module import is on the presentation/data allowlist;
//! 7. no platform window handle or Win32 module appears in the UI at all.
//!
//! [`run`] prints one row per file: the file, its production imports and the seven rule results.
//! Any failed rule makes the process exit non-zero. The companion tests feed synthetic violating
//! sources through the same checker, so a "passing" happy-path grep is not what certifies the
//! audit: the checker is proven to reject each violation it claims to catch.

use std::path::{Path, PathBuf};

/// Engine types a presentation module may name: plain data captured by the execution host.
const ENGINE_DATA_TYPES: &[&str] = &[
    "EngineState",
    "EngineFunction",
    "EngineVariable",
    "FunctionOrigin",
    "PlotCapture",
    "EngineError",
];

/// Execution types a presentation module may name: event/snapshot payloads, never the host.
const EXECUTION_DATA_TYPES: &[&str] = &[
    "Diagnostic",
    "DocumentId",
    "SourceRevision",
    "WorkspaceSnapshot",
    "RunId",
];

/// Framework-independent modules the UI may import. `engine` and `execution` are handled by the
/// narrower data-type rules above; everything else here is presentation, settings, document or
/// message/state code with no evaluation entry point.
const ALLOWED_CRATE_MODULES: &[&str] = &[
    "completion",
    "diagnostic_map",
    "document_io",
    "find",
    "glyphs",
    "history",
    "inspector",
    "keyboard",
    "language",
    "log_kind",
    "platform_dialogs",
    "quick_eval",
    "recovery",
    "results_tree",
    "run_state",
    "run_trace",
    "settings",
    "svg",
    // SYM-006: reads the engine's published `status`/`complete`/`exact` JSON fields to classify
    // a result. It holds no evaluator, constructs no engine and calls no compile path.
    "symbolic_status",
    "ui",
    "workbench",
    "workspace_table",
];

const RULES: &[&str] = &[
    "no_evaluation_call",
    "no_engine_construction",
    "no_compile_call",
    "engine_imports_are_data_only",
    "execution_imports_are_data_only",
    "crate_imports_allowlisted",
    "no_platform_window_handle",
];

/// One checked rule and what the scan observed.
#[derive(Clone, Debug)]
pub struct RuleOutcome {
    /// Stable rule name printed in the audit.
    pub rule: &'static str,
    /// True when the production source satisfies the rule.
    pub pass: bool,
    /// Short evidence: the offending token, or the observed import set.
    pub detail: String,
}

/// One file's audit row.
#[derive(Clone, Debug)]
pub struct FileReport {
    /// Path relative to the UI root.
    pub file: String,
    /// Production `use` items, normalized to single spaces.
    pub imports: Vec<String>,
    /// The seven rule outcomes, in [`RULES`] order.
    pub rules: Vec<RuleOutcome>,
}

impl FileReport {
    /// True when every rule passed.
    pub fn pass(&self) -> bool {
        self.rules.iter().all(|rule| rule.pass)
    }

    /// The imports as one printable cell.
    pub fn imports_cell(&self) -> String {
        if self.imports.is_empty() {
            "-".to_string()
        } else {
            self.imports.join(" ; ")
        }
    }

    /// The rules as one printable cell.
    pub fn rules_cell(&self) -> String {
        self.rules
            .iter()
            .map(|rule| format!("{}={}", rule.rule, if rule.pass { "PASS" } else { "FAIL" }))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// A whole-tree audit.
#[derive(Clone, Debug)]
pub struct AuditReport {
    /// The directory that was scanned.
    pub root: PathBuf,
    /// One row per `.rs` file, sorted by path.
    pub files: Vec<FileReport>,
    /// Setup failures (an unreadable file, a malformed source), which also fail the audit.
    pub errors: Vec<String>,
}

impl AuditReport {
    /// True when every file passed and there were no setup errors.
    pub fn pass(&self) -> bool {
        self.errors.is_empty() && self.files.iter().all(FileReport::pass)
    }
}

/// Audits every `.rs` file below `root` (recursively).
pub fn audit_dir(root: &Path) -> AuditReport {
    let mut paths = Vec::new();
    if let Err(error) = collect_rust_files(root, &mut paths) {
        return AuditReport {
            root: root.to_path_buf(),
            files: Vec::new(),
            errors: vec![format!("cannot read {}: {error}", root.display())],
        };
    }
    paths.sort();
    let mut files = Vec::new();
    let mut errors = Vec::new();
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string()
            .replace('\\', "/");
        match std::fs::read_to_string(&path) {
            Ok(source) => files.push(check_text(&relative, &source)),
            Err(error) => errors.push(format!("{relative}: {error}")),
        }
    }
    AuditReport {
        root: root.to_path_buf(),
        files,
        errors,
    }
}

fn collect_rust_files(root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// Checks one source text. `name` is only used in the returned row.
///
/// This is the single implementation the shipped audit and the negative tests share.
pub fn check_text(name: &str, source: &str) -> FileReport {
    let production = strip_test_modules(&strip_comments(source));
    let imports = collect_use_items(&production);

    // Gather every engine/execution name the file names, both through `use` and through a fully
    // qualified path. A name that appears only in a comment or a test module is already gone.
    let engine_names = engine_names(&imports, &production);
    let execution_names = execution_names(&imports, &production);
    let module_names = crate_module_roots(&imports, &production);

    let bad_engine: Vec<String> = engine_names
        .iter()
        .filter(|name| !ENGINE_DATA_TYPES.contains(&name.as_str()))
        .cloned()
        .collect();
    let bad_execution: Vec<String> = execution_names
        .iter()
        .filter(|name| !EXECUTION_DATA_TYPES.contains(&name.as_str()))
        .cloned()
        .collect();
    let bad_modules: Vec<String> = module_names
        .iter()
        .filter(|name| !ALLOWED_CRATE_MODULES.contains(&name.as_str()))
        .cloned()
        .collect();

    let eval = find_call(&production, "eval")
        .or_else(|| find_call(&production, "eval_full"));
    let construction = find_any(
        &production,
        &[
            "Engine::new",
            "RealEngine::new",
            "FakeEngine::new",
            "default_engine_factory",
        ],
    );
    let compile = find_call(&production, "compile");
    let platform = find_any(
        &production,
        &[
            "win32",
            "HWND",
            "hWnd",
            "windows_sys",
            "windows::Win32",
            "std::os::windows",
        ],
    );

    let mut rules = Vec::new();
    rules.push(outcome(
        "no_evaluation_call",
        eval.is_none(),
        eval.map(|token| format!("found {token:?}"))
            .unwrap_or_else(|| "no eval call".to_string()),
    ));
    rules.push(outcome(
        "no_engine_construction",
        construction.is_none(),
        construction
            .map(|token| format!("found {token:?}"))
            .unwrap_or_else(|| "no engine constructor".to_string()),
    ));
    rules.push(outcome(
        "no_compile_call",
        compile.is_none(),
        compile
            .map(|token| format!("found {token:?}"))
            .unwrap_or_else(|| "no compile call".to_string()),
    ));
    rules.push(outcome(
        "engine_imports_are_data_only",
        bad_engine.is_empty(),
        if bad_engine.is_empty() {
            format!(
                "engine names: {}",
                if engine_names.is_empty() {
                    "-".to_string()
                } else {
                    engine_names.join(",")
                }
            )
        } else {
            format!("non-data engine names: {}", bad_engine.join(","))
        },
    ));
    rules.push(outcome(
        "execution_imports_are_data_only",
        bad_execution.is_empty(),
        if bad_execution.is_empty() {
            format!(
                "execution names: {}",
                if execution_names.is_empty() {
                    "-".to_string()
                } else {
                    execution_names.join(",")
                }
            )
        } else {
            format!("non-data execution names: {}", bad_execution.join(","))
        },
    ));
    rules.push(outcome(
        "crate_imports_allowlisted",
        bad_modules.is_empty(),
        if bad_modules.is_empty() {
            format!(
                "crate modules: {}",
                if module_names.is_empty() {
                    "-".to_string()
                } else {
                    module_names.join(",")
                }
            )
        } else {
            format!("unexpected crate modules: {}", bad_modules.join(","))
        },
    ));
    rules.push(outcome(
        "no_platform_window_handle",
        platform.is_none(),
        platform
            .map(|token| format!("found {token:?}"))
            .unwrap_or_else(|| "no platform handle".to_string()),
    ));

    debug_assert_eq!(rules.len(), RULES.len());
    FileReport {
        file: name.to_string(),
        imports,
        rules,
    }
}

fn outcome(rule: &'static str, pass: bool, detail: String) -> RuleOutcome {
    RuleOutcome { rule, pass, detail }
}

/// Runs the ARC-003 audit over the shipped UI source and returns the process exit code.
pub fn run() -> i32 {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("ui");
    let report = audit_dir(&root);
    println!("lovelace-desktop --ui-audit (ARC-003)");
    println!("root: {}", report.root.display());
    println!(
        "rule: every production `src/ui/**` file must not evaluate a script, construct an engine or \
         invoke the compiler; engine/execution imports are message/state data only and every crate \
         import is allowlisted."
    );
    println!();
    println!("file | imports | rules | verdict");
    for file in &report.files {
        println!(
            "{} | {} | {} | {}",
            file.file,
            file.imports_cell(),
            file.rules_cell(),
            if file.pass() { "PASS" } else { "FAIL" }
        );
        for rule in &file.rules {
            if !rule.pass {
                println!("  FAILED {}: {}", rule.rule, rule.detail);
            }
        }
    }
    for error in &report.errors {
        println!("ERROR {error}");
    }
    let failed = report.files.iter().filter(|file| !file.pass()).count();
    println!();
    println!(
        "summary files={} passed={} failed={} errors={}",
        report.files.len(),
        report.files.len() - failed,
        failed,
        report.errors.len()
    );
    if report.pass() {
        println!("result PASS");
        0
    } else {
        println!("result FAIL");
        1
    }
}

// ---------------------------------------------------------------------------------------------
// Source cleaning
// ---------------------------------------------------------------------------------------------

/// Removes `//`, `/* */` comments while preserving string, char and raw-string literals.
pub(crate) fn strip_comments(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            let mut depth = 1usize;
            while i < chars.len() && depth > 0 {
                if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    if chars[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
        } else if c == '"' {
            i = copy_quoted(&chars, i, '"', &mut out);
        } else if c == '\'' && is_char_literal(&chars, i) {
            i = copy_quoted(&chars, i, '\'', &mut out);
        } else if c == 'r' && raw_string_start(&chars, i).is_some() {
            i = copy_raw_string(&chars, i, &mut out);
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn copy_quoted(chars: &[char], mut i: usize, quote: char, out: &mut String) -> usize {
    out.push(chars[i]);
    i += 1;
    while i < chars.len() {
        out.push(chars[i]);
        if chars[i] == '\\' {
            i += 1;
            if i < chars.len() {
                out.push(chars[i]);
                i += 1;
            }
        } else if chars[i] == quote {
            i += 1;
            break;
        } else {
            i += 1;
        }
    }
    i
}

fn is_char_literal(chars: &[char], i: usize) -> bool {
    // `'a'` has a closing quote within a handful of characters; a lifetime `'a` does not.
    let mut j = i + 1;
    let mut steps = 0;
    while j < chars.len() && steps < 6 && chars[j] != '\n' {
        if chars[j] == '\\' {
            j += 2;
            steps += 2;
            continue;
        }
        if chars[j] == '\'' {
            return true;
        }
        j += 1;
        steps += 1;
    }
    false
}

fn raw_string_start(chars: &[char], i: usize) -> Option<(usize, usize)> {
    if chars.get(i) != Some(&'r') {
        return None;
    }
    let mut j = i + 1;
    let mut hashes = 0usize;
    while j < chars.len() && chars[j] == '#' {
        hashes += 1;
        j += 1;
    }
    if chars.get(j) == Some(&'"') {
        Some((j, hashes))
    } else {
        None
    }
}

fn copy_raw_string(chars: &[char], i: usize, out: &mut String) -> usize {
    let (quote_at, hashes) = raw_string_start(chars, i).expect("caller checked the prefix");
    for c in &chars[i..=quote_at] {
        out.push(*c);
    }
    let mut j = quote_at + 1;
    while j < chars.len() {
        if chars[j] == '"'
            && (0..hashes).all(|offset| chars.get(j + 1 + offset) == Some(&'#'))
        {
            for c in &chars[j..=j + hashes] {
                out.push(*c);
            }
            return j + hashes + 1;
        }
        out.push(chars[j]);
        j += 1;
    }
    chars.len()
}

/// Replaces the contents of every string, char and raw-string literal with spaces, preserving
/// newlines. Rules scan code after this pass so a diagnostic string that names a prohibited API
/// (this audit's own negative cases, for example) is not mistaken for a call site.
pub(crate) fn mask_literals(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            let end = quoted_end(&chars, i, '"');
            for index in i..end {
                out.push(if chars[index] == '\n' { '\n' } else { ' ' });
            }
            i = end;
        } else if c == '\'' && is_char_literal(&chars, i) {
            let end = quoted_end(&chars, i, '\'');
            for index in i..end {
                out.push(if chars[index] == '\n' { '\n' } else { ' ' });
            }
            i = end;
        } else if c == 'r' && raw_string_start(&chars, i).is_some() {
            let (quote_at, hashes) = raw_string_start(&chars, i).expect("prefix checked");
            let end = raw_string_end(&chars, quote_at, hashes);
            for index in i..end {
                out.push(if chars[index] == '\n' { '\n' } else { ' ' });
            }
            i = end;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn quoted_end(chars: &[char], mut i: usize, quote: char) -> usize {
    i += 1;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == quote {
            return i + 1;
        }
        i += 1;
    }
    chars.len()
}

fn raw_string_end(chars: &[char], quote_at: usize, hashes: usize) -> usize {
    let mut j = quote_at + 1;
    while j < chars.len() {
        if chars[j] == '"' && (0..hashes).all(|offset| chars.get(j + 1 + offset) == Some(&'#')) {
            return j + hashes + 1;
        }
        j += 1;
    }
    chars.len()
}

/// Removes `#[cfg(test)] mod … { … }` blocks, leaving every production item.
pub(crate) fn strip_test_modules(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    loop {
        let Some(position) = rest.find("#[cfg(test)]") else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..position]);
        let after_attribute = position + "#[cfg(test)]".len();
        let tail = &rest[after_attribute..];
        let trimmed = tail.trim_start();
        if trimmed.starts_with("mod ") || trimmed.starts_with("pub mod ") {
            let offset = tail.len() - trimmed.len();
            let module_start = after_attribute + offset;
            if let Some(open) = rest[module_start..].find('{') {
                let open = module_start + open;
                let close = matching_brace(rest, open);
                rest = &rest[close..];
            } else {
                out.push_str(&rest[position..after_attribute]);
                rest = tail;
            }
        } else {
            out.push_str("#[cfg(test)]");
            rest = tail;
        }
    }
    out
}

/// Index just past the matching `}` of the `{` at `open`, string/lifetime/raw-string aware.
fn matching_brace(source: &str, open: usize) -> usize {
    let chars: Vec<char> = source.chars().collect();
    // Echo the byte index into a character index; `source` is ASCII for every brace we match,
    // and the caller only passes an index returned by `find`.
    let open_char = source[..open].chars().count();
    let mut depth = 0usize;
    let mut i = open_char;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            i = quoted_end(&chars, i, '"');
            continue;
        }
        if c == '\'' && is_char_literal(&chars, i) {
            i = quoted_end(&chars, i, '\'');
            continue;
        }
        if c == 'r' && raw_string_start(&chars, i).is_some() {
            let (quote_at, hashes) = raw_string_start(&chars, i).expect("prefix checked");
            i = raw_string_end(&chars, quote_at, hashes);
            continue;
        }
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let consumed: usize = chars[..=i].iter().map(|c| c.len_utf8()).sum();
                    return consumed;
                }
            }
            _ => {}
        }
        i += 1;
    }
    source.len()
}

// ---------------------------------------------------------------------------------------------
// Import scanning
// ---------------------------------------------------------------------------------------------

/// Every `use …;` item, normalized to single spaces.
fn collect_use_items(source: &str) -> Vec<String> {
    let chars: Vec<char> = source.chars().collect();
    let mut items = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if starts_word(&chars, i, "use") {
            let mut j = i + 3;
            let mut text = String::new();
            let mut in_string = false;
            while j < chars.len() {
                let c = chars[j];
                if in_string {
                    text.push(c);
                    if c == '\\' {
                        j += 1;
                        if j < chars.len() {
                            text.push(chars[j]);
                        }
                    } else if c == '"' {
                        in_string = false;
                    }
                } else {
                    match c {
                        ';' => {
                            j += 1;
                            break;
                        }
                        '"' => {
                            in_string = true;
                            text.push(c);
                        }
                        _ => text.push(c),
                    }
                }
                j += 1;
            }
            let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !normalized.is_empty() {
                items.push(normalized);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    items
}

fn starts_word(chars: &[char], i: usize, word: &str) -> bool {
    let word_chars: Vec<char> = word.chars().collect();
    if i + word_chars.len() > chars.len() || chars[i..i + word_chars.len()] != word_chars[..] {
        return false;
    }
    if i > 0 && is_ident(chars[i - 1]) {
        return false;
    }
    if let Some(next) = chars.get(i + word_chars.len()) {
        if is_ident(*next) {
            return false;
        }
    }
    true
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Engine type names imported or named through a `crate::engine::…` path.
fn engine_names(imports: &[String], production: &str) -> Vec<String> {
    let mut names = Vec::new();
    for item in imports {
        if item.contains("crate::engine::") {
            names.extend(names_in_use_item(item));
        }
    }
    names.extend(path_names(production, "crate::engine::"));
    sort_unique(&mut names);
    names
}

fn execution_names(imports: &[String], production: &str) -> Vec<String> {
    let mut names = Vec::new();
    for item in imports {
        if item.contains("crate::execution::") {
            names.extend(names_in_use_item(item));
        }
    }
    names.extend(path_names(production, "crate::execution::"));
    sort_unique(&mut names);
    names
}

fn crate_module_roots(imports: &[String], production: &str) -> Vec<String> {
    let mut names = Vec::new();
    for item in imports {
        if let Some(after) = item.split("crate::").nth(1) {
            if let Some(root) = after.split("::").next() {
                let root = root.trim_matches(|c: char| !is_ident(c));
                if !root.is_empty() && root != "engine" && root != "execution" {
                    names.push(root.to_string());
                }
            }
        }
    }
    for prefix in ["crate::"] {
        let mut search = production;
        while let Some(position) = search.find(prefix) {
            let after = &search[position + prefix.len()..];
            let root: String = after
                .chars()
                .take_while(|c| is_ident(*c))
                .collect();
            let advance = root.len().max(1);
            if !root.is_empty()
                && root != "engine"
                && root != "execution"
                && crate_module_roots_is_not_use(&root)
            {
                names.push(root);
            }
            search = &after[advance..];
        }
    }
    sort_unique(&mut names);
    names
}

/// `crate::engine` and `crate::execution` are handled by their own rules; every other root is a
/// presentation/data module. A `crate::` occurrence in a `use` item would be counted twice, but
/// the duplicate is harmless and hidden by `sort_unique`.
fn crate_module_roots_is_not_use(_root: &str) -> bool {
    true
}

fn names_in_use_item(item: &str) -> Vec<String> {
    // `crate::engine::{EngineFunction, EngineState}` or `crate::engine::EngineState`.
    let after = item
        .split("crate::engine::")
        .nth(1)
        .or_else(|| item.split("crate::execution::").nth(1))
        .unwrap_or_default();
    let mut names = Vec::new();
    if after.trim_start().starts_with('{') {
        let inner = after
            .split_once('{')
            .map(|(_, tail)| tail)
            .unwrap_or_default();
        let inner = inner.split('}').next().unwrap_or_default();
        for part in inner.split(',') {
            let name = part.trim().trim_start_matches("self::");
            let name = name.split("::").next().unwrap_or_default().trim();
            if !name.is_empty() && name != "self" && name != "*" {
                names.push(name.to_string());
            }
        }
    } else {
        let name = after
            .split(|c: char| c == ';' || c == ',' || c == '}' || c.is_whitespace())
            .next()
            .unwrap_or_default()
            .trim()
            .trim_end_matches("as");
        if !name.is_empty() {
            names.push(name.to_string());
        }
    }
    names
}

/// Names after every `prefix` occurrence, e.g. `crate::engine::EngineState::…` yields
/// `EngineState`.
fn path_names(source: &str, prefix: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut search = source;
    while let Some(position) = search.find(prefix) {
        let after = &search[position + prefix.len()..];
        let name: String = after.chars().take_while(|c| is_ident(*c)).collect();
        let advance = name.len().max(1);
        if !name.is_empty() {
            names.push(name);
        }
        search = &after[advance..];
    }
    names
}

fn sort_unique(names: &mut Vec<String>) {
    names.sort();
    names.dedup();
}

/// Finds a function call named `name` (`name(`, `.name(`, `name (`) outside strings.
fn find_call(source: &str, name: &str) -> Option<String> {
    let chars: Vec<char> = source.chars().collect();
    let name_chars: Vec<char> = name.chars().collect();
    let mut i = 0usize;
    while i + name_chars.len() <= chars.len() {
        if chars[i..i + name_chars.len()] == name_chars[..]
            && (i == 0 || !is_ident(chars[i - 1]))
        {
            let mut j = i + name_chars.len();
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if chars.get(j) == Some(&'(') {
                let start = i.saturating_sub(1);
                let token: String = chars[start..j + 1].iter().collect();
                return Some(token.trim().to_string());
            }
            i += name_chars.len();
        } else {
            i += 1;
        }
    }
    None
}

fn find_any(source: &str, needles: &[&str]) -> Option<String> {
    needles
        .iter()
        .find(|needle| source.contains(**needle))
        .map(|needle| (*needle).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_ui_passes_the_separation_audit() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("ui");
        let report = audit_dir(&root);
        assert!(report.errors.is_empty(), "setup errors: {:?}", report.errors);
        assert!(
            report.files.len() >= 15,
            "the audit must scan every UI file, found {}",
            report.files.len()
        );
        let failed: Vec<&FileReport> = report.files.iter().filter(|file| !file.pass()).collect();
        assert!(
            failed.is_empty(),
            "ARC-003 violations: {:#?}",
            failed
                .iter()
                .flat_map(|file| file.rules.iter().filter(|rule| !rule.pass))
                .collect::<Vec<_>>()
        );
        assert!(report.pass());
    }

    #[test]
    fn the_audit_rejects_an_evaluation_call() {
        let report = check_text("bad.rs", "fn paint() { let _ = engine.eval(\"a = 1\"); }");
        assert!(!report.pass());
        assert!(failed_rule(&report, "no_evaluation_call"));
    }

    #[test]
    fn the_audit_rejects_engine_construction_and_the_compiler() {
        for source in [
            "fn paint() { let _ = RealEngine::new(); }",
            "fn paint() { let _ = FakeEngine::new(); }",
            "fn paint() { let _ = Engine::new(); }",
            "fn paint() { let _ = engine::default_engine_factory(); }",
        ] {
            let report = check_text("bad.rs", source);
            assert!(
                !report.pass(),
                "the construction audit passed {source:?}"
            );
            assert!(failed_rule(&report, "no_engine_construction"));
        }
        let report = check_text("bad.rs", "fn paint() { compile(x); }");
        assert!(!report.pass());
        assert!(failed_rule(&report, "no_compile_call"));
    }

    #[test]
    fn the_audit_rejects_an_evaluator_import_and_a_non_data_engine() {
        let report = check_text(
            "bad.rs",
            "use crate::engine::{Evaluator, RealEngine};\nfn paint() {}",
        );
        assert!(!report.pass());
        assert!(failed_rule(&report, "engine_imports_are_data_only"));
        let report = check_text(
            "bad.rs",
            "use crate::engine::RealEngine;\nfn paint() {}",
        );
        assert!(!report.pass());
        assert!(failed_rule(&report, "engine_imports_are_data_only"));
    }

    #[test]
    fn the_audit_rejects_the_execution_host_and_unlisted_modules() {
        let report = check_text(
            "bad.rs",
            "use crate::execution::{ExecutionHost, DocumentId};\nfn paint() {}",
        );
        assert!(!report.pass());
        assert!(failed_rule(&report, "execution_imports_are_data_only"));
        let report = check_text("bad.rs", "use crate::win32::shell;\nfn paint() {}");
        assert!(!report.pass());
        assert!(failed_rule(&report, "crate_imports_allowlisted"));
        assert!(failed_rule(&report, "no_platform_window_handle"));
    }

    #[test]
    fn comments_and_test_modules_are_not_production() {
        let source = "\
// engine.eval(\"a = 1\") in a comment is not a production call
/* RealEngine::new() here is also a comment */
fn production_ok() { let _ = 1 + 1; }

#[cfg(test)]
mod tests {
    use crate::engine::Evaluator;
    fn evaluates() { let _ = crate::engine::RealEngine::new().eval(\"1\"); }
}
";
        let report = check_text("ok.rs", source);
        assert!(
            report.pass(),
            "comments/test code must be stripped, got {:#?}",
            report
        );
    }

    #[test]
    fn every_rule_has_a_negative_case_except_the_two_data_type_rules() {
        // A guard against deleting the negative tests: one synthetic violation per syntactic
        // rule, checked through the same `check_text` entry point the shipped audit uses.
        let cases = [
            ("no_evaluation_call", "fn x(){ engine.eval(\"1\"); }"),
            ("no_engine_construction", "fn x(){ RealEngine::new(); }"),
            ("no_compile_call", "fn x(){ compile(x); }"),
            (
                "engine_imports_are_data_only",
                "use crate::engine::Evaluator; fn x(){}",
            ),
            (
                "execution_imports_are_data_only",
                "use crate::execution::ExecutionHost; fn x(){}",
            ),
            (
                "crate_imports_allowlisted",
                "use crate::win32::shell; fn x(){}",
            ),
            (
                "no_platform_window_handle",
                "fn x(){ let _: HWND = 0; }",
            ),
        ];
        for (rule, source) in cases {
            let report = check_text("bad.rs", source);
            assert!(
                failed_rule(&report, rule),
                "rule {rule} did not reject {source:?}: {:#?}",
                report
            );
        }
    }

    fn failed_rule(report: &FileReport, rule: &str) -> bool {
        report
            .rules
            .iter()
            .any(|outcome| outcome.rule == rule && !outcome.pass)
    }
}
