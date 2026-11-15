//! What `Engine::new()` loads, what `Engine::new_bare()` withholds, and how a family is re-enabled.
//!
//! Run from the repository root:
//!     cargo run -p lovelace-lang --example bare_engine

use lovelace_lang::Engine;
use lovelace_lang::LangError;
use lovelace_lang::modus::{ModusHost, ModusPlugin};

/// A plugin that registers nothing and only carries a family's published name. The registry gate in
/// `Engine::plugin_enabled` keys on that name, so loading it re-exposes the engine's own builtins
/// for the family without re-implementing any of them.
struct EnableFamily(&'static str);

impl ModusPlugin for EnableFamily {
    fn name(&self) -> &str {
        self.0
    }

    fn register(&mut self, _host: &mut ModusHost) -> Result<(), LangError> {
        Ok(())
    }
}

fn report(label: &str, engine: &mut Engine, source: &str) {
    match engine.eval(source) {
        Ok(value) => println!("{label:<10} {source:<24} => {}", value.display()),
        Err(error) => println!(
            "{label:<10} {source:<24} => [{}] {}",
            error.code, error.message
        ),
    }
}

fn main() {
    println!("-- Engine::new() --");
    let mut full = Engine::new();
    report("new", &mut full, "fft([0, 1, 0, 0])");
    report("new", &mut full, "det([[1, 2], [3, 4]])");
    report("new", &mut full, "simplify(x/x)");
    report("new", &mut full, "symbol(\"x\")");
    report("new", &mut full, "lower(1, [])");

    println!("-- Engine::new_bare() --");
    let mut bare = Engine::new_bare();
    report("bare", &mut bare, "abs(-3)");
    report("bare", &mut bare, "det([[1, 2], [3, 4]])");
    report("bare", &mut bare, "fft([0, 1, 0, 0])");
    report("bare", &mut bare, "symbol(\"x\")");
    report("bare", &mut bare, "lower(1, [])");

    println!("-- new_bare() + load_plugin(EnableFamily(\"Lovelace.Dsp\")) --");
    let mut dsp = Engine::new_bare();
    report("bare", &mut dsp, "fft([0, 1, 0, 0])");
    dsp.load_plugin(&mut EnableFamily("Lovelace.Dsp"))
        .expect("the plugin loads once");
    report("bare+dsp", &mut dsp, "fft([0, 1, 0, 0])");
    report("bare+dsp", &mut dsp, "det([[1, 2], [3, 4]])");
    report("bare+dsp", &mut dsp, "symbol(\"x\")");
    match dsp.load_plugin(&mut EnableFamily("Lovelace.Dsp")) {
        Ok(()) => println!("second load: unexpectedly succeeded"),
        Err(error) => println!("second load: [{}] {}", error.code, error.message),
    }

    println!("-- new_bare() + load_plugin(EnableFamily(\"Lovelace.Symbolics\")) --");
    let mut sym = Engine::new_bare();
    report("bare", &mut sym, "symbol(\"x\")");
    sym.load_plugin(&mut EnableFamily("Lovelace.Symbolics"))
        .expect("the plugin loads once");
    report("bare+sym", &mut sym, "symbol(\"x\")");
    report("bare+sym", &mut sym, "diff(x^2, x)");

    println!("-- new_bare() + load_plugin(EnableFamily(\"Lovelace.MathIR\")) --");
    let mut ir = Engine::new_bare();
    report("bare", &mut ir, "lower(1, [])");
    ir.load_plugin(&mut EnableFamily("Lovelace.MathIR"))
        .expect("the plugin loads once");
    report("bare+ir", &mut ir, "lower(1, [])");
}
