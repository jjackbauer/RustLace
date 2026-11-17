//! The `sys_*` adapter: Lovelace values to `lovelace-systems` models and back.
//!
//! This module is deliberately thin. It parses record/vector/scalar values into the systems
//! crate's typed model, calls the crate and builds structured `Value::Record`/`Value::Vector`
//! results. It implements no formula, keeps no state and performs no floating-point conversion.
//! Every number crosses as an exact `Q`; the display precision only decides how a `Q` is rendered
//! as a `Real` in the returned record.
//!
//! Cancellation is forwarded to the engine's own evaluation scope, and a systems refusal is
//! republished as a structured `LangError` with the same stable code and the mapped category.

use lovelace_nat::Nat;
use lovelace_real::Real;
use lovelace_systems::{
    Cancel, ClosedWorkload, OpenWorkload, Q, QueueNetwork, ServiceDiscipline, SolverOptions,
    Station, SystemsError, Workload, analyze, analyze_closed, bottlenecks, mm_infinity, mm1, mmc,
    sweep_arrival, sweep_population,
};

use crate::{Engine, LangError, Value};

/// The default decimal rendering precision of a systems result (the DSP fast default).
pub const DEFAULT_PRECISION: usize = 30;

/// The cancellation bridge: the adapter polls the engine's per-evaluation scope.
struct EngineCancel;

impl Cancel for EngineCancel {
    fn is_cancelled(&self) -> bool {
        lovelace_symbolics::cancel::check()
    }
}

/// Dispatches one `sys_*` builtin.
pub fn call(
    _engine: &mut Engine,
    name: &str,
    args: Vec<Value>,
    pos: usize,
) -> Result<Value, LangError> {
    let ctx = Ctx { name, pos };
    match name {
        "sys_mm1" => {
            require_len(&args, 2, name, pos)?;
            let lambda = ctx.number(&args[0], 0)?;
            let mu = ctx.number(&args[1], 1)?;
            let result = mm1(&lambda, &mu, &options(None), &EngineCancel)
                .map_err(|error| systems_error(error, pos))?;
            Ok(station_analysis(&result, DEFAULT_PRECISION))
        }
        "sys_mmc" => {
            require_len(&args, 3, name, pos)?;
            let lambda = ctx.number(&args[0], 0)?;
            let mu = ctx.number(&args[1], 1)?;
            let servers = ctx.usize(&args[2], 2)?;
            let result = mmc(&lambda, &mu, servers, &options(None), &EngineCancel)
                .map_err(|error| systems_error(error, pos))?;
            Ok(station_analysis(&result, DEFAULT_PRECISION))
        }
        "sys_delay" => {
            require_len(&args, 2, name, pos)?;
            let lambda = ctx.number(&args[0], 0)?;
            let mu = ctx.number(&args[1], 1)?;
            let result = mm_infinity(&lambda, &mu, &options(None), &EngineCancel)
                .map_err(|error| systems_error(error, pos))?;
            Ok(station_analysis(&result, DEFAULT_PRECISION))
        }
        "sys_station" => {
            if args.len() < 3 || args.len() > 4 {
                return Err(arity_error(name, pos));
            }
            let id = ctx.text(&args[0], 0)?;
            let service_time = ctx.number(&args[1], 1)?;
            let visits = ctx.number(&args[2], 2)?;
            let discipline = match args.get(3) {
                None => ServiceDiscipline::FcfsSingleServer,
                Some(Value::Text(text)) if text.eq_ignore_ascii_case("delay") => {
                    ServiceDiscipline::Delay
                }
                Some(value) => {
                    let servers = ctx.usize(value, 3)?;
                    ServiceDiscipline::MultiServer { servers }
                }
            };
            let station = Station::new(id, discipline, service_time, visits)
                .map_err(|error| systems_error(error, pos))?;
            Ok(station_record(&station, DEFAULT_PRECISION))
        }
        "sys_open" => {
            require_len(&args, 2, name, pos)?;
            let rate = ctx.number(&args[0], 0)?;
            let stations = ctx.stations(&args[1], 1)?;
            let network = QueueNetwork::new(
                "open",
                Workload::Open(OpenWorkload { arrival_rate: rate }),
                stations,
            )
            .map_err(|error| systems_error(error, pos))?;
            Ok(network_record(&network, DEFAULT_PRECISION))
        }
        "sys_closed" => {
            require_len(&args, 3, name, pos)?;
            let population = ctx.count(&args[0], 0)?;
            let think_time = ctx.number(&args[1], 1)?;
            let stations = ctx.stations(&args[2], 2)?;
            let network = QueueNetwork::new(
                "closed",
                Workload::Closed(ClosedWorkload {
                    population,
                    think_time,
                }),
                stations,
            )
            .map_err(|error| systems_error(error, pos))?;
            Ok(network_record(&network, DEFAULT_PRECISION))
        }
        "sys_analyze" => {
            if args.is_empty() || args.len() > 2 {
                return Err(arity_error(name, pos));
            }
            let (network, precision) = parse_network(&ctx, &args[0], 0)?;
            let chosen = precision_from(&ctx, &args, 1)?.unwrap_or(precision);
            let result = analyze(&network, &options(Some(chosen)), &EngineCancel)
                .map_err(|error| systems_error(error, pos))?;
            Ok(analysis_record(&result, chosen))
        }
        "sys_mva" => {
            if args.is_empty() || args.len() > 2 {
                return Err(arity_error(name, pos));
            }
            let (network, precision) = parse_network(&ctx, &args[0], 0)?;
            let chosen = precision_from(&ctx, &args, 1)?.unwrap_or(precision);
            let result = analyze_closed(&network, &options(Some(chosen)), &EngineCancel)
                .map_err(|error| systems_error(error, pos))?;
            Ok(analysis_record(&result, chosen))
        }
        "sys_bottlenecks" => {
            require_len(&args, 1, name, pos)?;
            let (network, precision) = parse_network(&ctx, &args[0], 0)?;
            Ok(bottleneck_record(&bottlenecks(&network), precision))
        }
        "sys_sweep" => {
            require_len(&args, 4, name, pos)?;
            let (network, precision) = parse_network(&ctx, &args[0], 0)?;
            let from = ctx.number(&args[1], 1)?;
            let to = ctx.number(&args[2], 2)?;
            let step = ctx.number(&args[3], 3)?;
            let chosen = precision;
            let sweep = match &network.workload {
                Workload::Closed(_) => {
                    let from = ctx.count(&args[1], 1)?;
                    let to = ctx.count(&args[2], 2)?;
                    let step = ctx.count(&args[3], 3)?;
                    sweep_population(
                        &network,
                        from,
                        to,
                        step,
                        &options(Some(chosen)),
                        &EngineCancel,
                    )
                }
                Workload::Open(_) => sweep_arrival(
                    &network,
                    &from,
                    &to,
                    &step,
                    &options(Some(chosen)),
                    &EngineCancel,
                ),
            }
            .map_err(|error| systems_error(error, pos))?;
            Ok(sweep_record(&sweep, chosen))
        }
        other => Err(LangError::coded(
            format!("Unknown systems function '{other}'."),
            pos,
            "UnknownFunction",
            "UnsupportedOperation",
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// Value -> model
// ---------------------------------------------------------------------------------------------

/// The current call: its registered name and the caller's source position.
struct Ctx<'a> {
    name: &'a str,
    pos: usize,
}

impl Ctx<'_> {
    /// The documented argument-refusal grammar: `<builtin>(): argument <n> expects <x>; got <y>.`
    fn error(
        &self,
        index: usize,
        expected: impl std::fmt::Display,
        arrived: impl std::fmt::Display,
    ) -> LangError {
        LangError::coded(
            format!(
                "{}(): argument {} expects {expected}; got {arrived}.",
                self.name,
                index + 1
            ),
            self.pos,
            "InvalidArgument",
            "TypeMismatch",
        )
    }

    fn text(&self, value: &Value, index: usize) -> Result<String, LangError> {
        match value {
            Value::Text(text) => Ok(text.clone()),
            other => Err(self.error(index, "text", other.kind_name())),
        }
    }

    fn number(&self, value: &Value, index: usize) -> Result<Q, LangError> {
        match value {
            Value::Natural(n) => Ok(Q::from_nat(n)),
            Value::Integer(i) => Ok(Q::from_int(i)),
            Value::Real(real) => {
                // A systems input is an exact model parameter. A Real is accepted only when its
                // stored digits denote the input without a prior truncation; a precision-limited
                // value would otherwise be silently promoted to an exact rational (RNL-NUM-009).
                let conversion = real.to_rat();
                if conversion.is_lossless() {
                    Ok(conversion.value)
                } else {
                    Err(self.error(
                        index,
                        "an exact number (an integer, fraction or exact decimal)",
                        format!("a precision-limited Real `{real}`"),
                    ))
                }
            }
            other => Err(self.error(index, "a number", other.kind_name())),
        }
    }

    fn count(&self, value: &Value, index: usize) -> Result<u64, LangError> {
        let number = self.number(value, index)?;
        number
            .to_i64()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| self.error(index, "a non-negative integer", number.to_string()))
    }

    fn usize(&self, value: &Value, index: usize) -> Result<usize, LangError> {
        let count = self.count(value, index)?;
        usize::try_from(count)
            .map_err(|_| self.error(index, "a non-negative integer that fits this host", count))
    }

    fn precision(&self, value: &Value, index: usize) -> Result<usize, LangError> {
        let precision = self.usize(value, index)?;
        if !(1..=100_000).contains(&precision) {
            return Err(self.error(
                index,
                "a precision between 1 and 100000 decimal places",
                precision,
            ));
        }
        Ok(precision)
    }

    fn stations(&self, value: &Value, index: usize) -> Result<Vec<Station>, LangError> {
        match value {
            Value::Vector(items) => items
                .iter()
                .map(|item| parse_station(self, item, index))
                .collect(),
            other => Err(self.error(index, "a vector of station records", other.kind_name())),
        }
    }

    fn record<'v>(
        &self,
        value: &'v Value,
        index: usize,
        expected: &str,
    ) -> Result<&'v [(String, Value)], LangError> {
        match value {
            Value::Record(type_name, fields) if type_name == expected => Ok(fields),
            Value::Record(type_name, _) => Err(self.error(
                index,
                format!("a {expected} record"),
                format!("a {type_name} record"),
            )),
            other => Err(self.error(index, format!("a {expected} record"), other.kind_name())),
        }
    }
}

fn require_len(args: &[Value], expected: usize, name: &str, pos: usize) -> Result<(), LangError> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(LangError::coded(
            format!(
                "'{name}' expects {expected} argument(s), but {} were given.",
                args.len()
            ),
            pos,
            "InvalidArgument",
            "TypeMismatch",
        ))
    }
}

fn arity_error(name: &str, pos: usize) -> LangError {
    LangError::coded(
        format!("'{name}' was called with an unsupported argument count."),
        pos,
        "InvalidArgument",
        "TypeMismatch",
    )
}

fn parse_station(ctx: &Ctx<'_>, value: &Value, index: usize) -> Result<Station, LangError> {
    let fields = ctx.record(value, index, "Station")?;
    let id = ctx.text(field(ctx, fields, "id", index)?, index)?;
    let discipline = ctx.text(field(ctx, fields, "discipline", index)?, index)?;
    let service_time = ctx.number(field(ctx, fields, "service_time", index)?, index)?;
    let visits = ctx.number(field(ctx, fields, "visits", index)?, index)?;
    let discipline = match discipline.as_str() {
        "fcfs" => ServiceDiscipline::FcfsSingleServer,
        "delay" => ServiceDiscipline::Delay,
        "mmc" => {
            let servers = ctx.usize(field(ctx, fields, "servers", index)?, index)?;
            ServiceDiscipline::MultiServer { servers }
        }
        _ => {
            return Err(ctx.error(
                index,
                "a service discipline of fcfs, delay or mmc",
                discipline,
            ));
        }
    };
    Station::new(id, discipline, service_time, visits)
        .map_err(|error| systems_error(error, ctx.pos))
}

fn parse_network(
    ctx: &Ctx<'_>,
    value: &Value,
    index: usize,
) -> Result<(QueueNetwork, usize), LangError> {
    let fields = ctx.record(value, index, "QueueNetwork")?;
    let name = fields
        .iter()
        .find(|(field, _)| field == "name")
        .map(|(_, value)| ctx.text(value, index))
        .transpose()?
        .unwrap_or_else(|| "network".to_string());
    let workload = ctx.text(field(ctx, fields, "workload", index)?, index)?;
    let stations = ctx.stations(field(ctx, fields, "stations", index)?, index)?;
    let workload = match workload.as_str() {
        "open" => Workload::Open(OpenWorkload {
            arrival_rate: ctx.number(field(ctx, fields, "arrival_rate", index)?, index)?,
        }),
        "closed" => Workload::Closed(ClosedWorkload {
            population: ctx.count(field(ctx, fields, "population", index)?, index)?,
            think_time: ctx.number(field(ctx, fields, "think_time", index)?, index)?,
        }),
        _ => {
            return Err(ctx.error(index, "a workload of open or closed", workload));
        }
    };
    let precision = fields
        .iter()
        .find(|(field, _)| field == "precision")
        .map(|(_, value)| ctx.precision(value, index))
        .transpose()?
        .unwrap_or(DEFAULT_PRECISION);
    let network = QueueNetwork::new(name, workload, stations)
        .map_err(|error| systems_error(error, ctx.pos))?;
    Ok((network, precision))
}

fn precision_from(ctx: &Ctx<'_>, args: &[Value], index: usize) -> Result<Option<usize>, LangError> {
    match args.get(index) {
        Some(value) => Ok(Some(ctx.precision(value, index)?)),
        None => Ok(None),
    }
}

fn field<'a>(
    ctx: &Ctx<'_>,
    fields: &'a [(String, Value)],
    name: &str,
    index: usize,
) -> Result<&'a Value, LangError> {
    fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value)
        .ok_or_else(|| {
            ctx.error(
                index,
                format!("a record with the required field '{name}'"),
                "a record without it",
            )
        })
}

fn options(precision: Option<usize>) -> SolverOptions {
    SolverOptions {
        precision: precision.unwrap_or(DEFAULT_PRECISION),
        ..SolverOptions::default()
    }
}

fn systems_error(error: SystemsError, pos: usize) -> LangError {
    LangError::coded(
        error.message().to_string(),
        pos,
        error.code(),
        error.category().as_str(),
    )
}

// ---------------------------------------------------------------------------------------------
// Model -> Value
// ---------------------------------------------------------------------------------------------

fn record(type_name: &str, fields: Vec<(&str, Value)>) -> Value {
    Value::Record(
        type_name.to_string(),
        fields
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    )
}

fn text_list(values: &[&str]) -> Value {
    Value::Vector(
        values
            .iter()
            .map(|value| Value::Text((*value).to_string()))
            .collect(),
    )
}

fn real(q: &Q, precision: usize) -> Value {
    // The projection is explicit: `Real::from_rational` reports whether the rendering budget
    // truncated the exact rational. The decimal field is a projection; the record's `exact`
    // sidecar carries the numerator/denominator pair.
    Value::Real(Real::from_rational(q, precision).value)
}

/// The structured exact value: numerator and denominator, both exact language values.
///
/// This is the RNL-NUM-011 representation. A decimal field such as `utilization` is a projection;
/// `exact.utilization` retains the exact `Q` across the language and wire boundaries without
/// introducing a new language kind (RNL-MIG-005 keeps the promotion policy unchanged).
fn exact_rational(value: &Q) -> Value {
    record(
        "ExactRational",
        vec![
            ("numerator", Value::Integer(value.numerator().clone())),
            ("denominator", Value::Natural(value.denominator().clone())),
        ],
    )
}

fn exact_values(pairs: Vec<(&'static str, &Q)>) -> Value {
    record(
        "ExactValues",
        pairs
            .into_iter()
            .map(|(name, value)| (name, exact_rational(value)))
            .collect(),
    )
}

fn count(value: u64) -> Value {
    Value::Natural(Nat::from_u64(value))
}

fn station_record(station: &Station, precision: usize) -> Value {
    let mut fields = vec![
        ("id", Value::Text(station.id.as_str().to_string())),
        (
            "discipline",
            Value::Text(station.discipline.as_str().to_string()),
        ),
        ("service_time", real(&station.service_time, precision)),
        ("visits", real(&station.visits, precision)),
        ("service_demand", real(&station.service_demand(), precision)),
    ];
    if let Some(servers) = station.servers() {
        fields.push(("servers", count(servers as u64)));
    }
    record("Station", fields)
}

fn network_record(network: &QueueNetwork, precision: usize) -> Value {
    let stations = Value::Vector(
        network
            .stations
            .iter()
            .map(|station| station_record(station, precision))
            .collect(),
    );
    let mut fields = vec![
        ("name", Value::Text(network.name.clone())),
        (
            "workload",
            Value::Text(network.workload.class().to_string()),
        ),
        ("stations", stations),
        ("precision", count(precision as u64)),
    ];
    match &network.workload {
        Workload::Open(open) => fields.push(("arrival_rate", real(&open.arrival_rate, precision))),
        Workload::Closed(closed) => {
            fields.push(("population", count(closed.population)));
            fields.push(("think_time", real(&closed.think_time, precision)));
        }
    }
    record("QueueNetwork", fields)
}

fn station_metrics_fields(
    station: &lovelace_systems::StationResult,
    precision: usize,
) -> Vec<(&'static str, Value)> {
    let mut fields = vec![
        ("id", Value::Text(station.id.clone())),
        ("discipline", Value::Text(station.discipline.to_string())),
        ("visits", real(&station.visits, precision)),
        ("service_time", real(&station.service_time, precision)),
        ("service_demand", real(&station.service_demand, precision)),
        ("throughput", real(&station.throughput, precision)),
        ("utilization", real(&station.utilization, precision)),
        ("busy_servers", real(&station.busy_servers, precision)),
        ("response_time", real(&station.response_time, precision)),
        ("waiting_time", real(&station.waiting_time, precision)),
        ("occupancy", real(&station.occupancy, precision)),
        ("queue_length", real(&station.queue_length, precision)),
    ];
    if let Some(servers) = station.servers {
        fields.push(("servers", count(servers as u64)));
    }
    if let Some(probability) = &station.probability_waiting {
        fields.push(("probability_waiting", real(probability, precision)));
    }
    let mut exact = vec![
        ("visits", &station.visits),
        ("service_time", &station.service_time),
        ("service_demand", &station.service_demand),
        ("throughput", &station.throughput),
        ("utilization", &station.utilization),
        ("busy_servers", &station.busy_servers),
        ("response_time", &station.response_time),
        ("waiting_time", &station.waiting_time),
        ("occupancy", &station.occupancy),
        ("queue_length", &station.queue_length),
    ];
    if let Some(probability) = &station.probability_waiting {
        exact.push(("probability_waiting", probability));
    }
    fields.push(("exact_values", exact_values(exact)));
    fields
}

fn station_metrics(station: &lovelace_systems::StationResult, precision: usize) -> Value {
    record("StationMetrics", station_metrics_fields(station, precision))
}

fn provenance_fields(
    provenance: &lovelace_systems::Provenance,
    precision: usize,
) -> Vec<(&'static str, Value)> {
    vec![
        ("model", Value::Text(provenance.model_class.to_string())),
        ("solver", Value::Text(provenance.solver_name.to_string())),
        (
            "solver_version",
            Value::Text(provenance.solver_version.to_string()),
        ),
        ("exact", Value::Boolean(provenance.exactness.is_exact())),
        (
            "exactness",
            Value::Text(provenance.exactness.as_str().to_string()),
        ),
        ("precision", count(precision as u64)),
        (
            "status",
            Value::Text(provenance.status.as_str().to_string()),
        ),
        ("assumptions", text_list(&provenance.assumptions)),
        (
            "approximation",
            match provenance.approximation {
                Some(name) => Value::Text(name.to_string()),
                None => Value::Void,
            },
        ),
        (
            "warnings",
            Value::Vector(
                provenance
                    .warnings
                    .iter()
                    .map(|warning| Value::Text(warning.clone()))
                    .collect(),
            ),
        ),
    ]
}

fn law_records(checks: &[lovelace_systems::LawCheck], precision: usize) -> Value {
    Value::Vector(
        checks
            .iter()
            .map(|check| {
                record(
                    "LawCheck",
                    vec![
                        ("name", Value::Text(check.name.to_string())),
                        ("left", real(&check.left, precision)),
                        ("right", real(&check.right, precision)),
                        ("holds", Value::Boolean(check.holds)),
                        (
                            "exact_values",
                            record(
                                "ExactLaw",
                                vec![
                                    ("left", exact_rational(&check.left)),
                                    ("right", exact_rational(&check.right)),
                                ],
                            ),
                        ),
                    ],
                )
            })
            .collect(),
    )
}

fn analysis_record(result: &lovelace_systems::AnalysisResult, precision: usize) -> Value {
    let mut fields = provenance_fields(&result.provenance, precision);
    fields.push(("throughput", real(&result.aggregate.throughput, precision)));
    fields.push((
        "response_time",
        real(&result.aggregate.response_time, precision),
    ));
    fields.push(("occupancy", real(&result.aggregate.occupancy, precision)));
    fields.push((
        "queue_length",
        real(&result.aggregate.queue_length, precision),
    ));
    if let Some(population) = result.aggregate.population {
        fields.push(("population", count(population)));
    }
    if let Some(think_time) = &result.aggregate.think_time {
        fields.push(("think_time", real(think_time, precision)));
    }
    let mut aggregate_exact = vec![
        ("throughput", &result.aggregate.throughput),
        ("response_time", &result.aggregate.response_time),
        ("occupancy", &result.aggregate.occupancy),
        ("queue_length", &result.aggregate.queue_length),
    ];
    if let Some(think_time) = &result.aggregate.think_time {
        aggregate_exact.push(("think_time", think_time));
    }
    fields.push(("exact_values", exact_values(aggregate_exact)));
    fields.push(("laws", law_records(&result.aggregate.law_checks, precision)));
    fields.push((
        "stations",
        Value::Vector(
            result
                .stations
                .iter()
                .map(|station| station_metrics(station, precision))
                .collect(),
        ),
    ));
    if let Some(points) = &result.populations {
        fields.push((
            "populations",
            Value::Vector(
                points
                    .iter()
                    .map(|point| {
                        record(
                            "PopulationPoint",
                            vec![
                                ("population", count(point.population)),
                                ("throughput", real(&point.throughput, precision)),
                                ("response_time", real(&point.response_time, precision)),
                                ("occupancy", real(&point.occupancy, precision)),
                                (
                                    "exact_values",
                                    exact_values(vec![
                                        ("throughput", &point.throughput),
                                        ("response_time", &point.response_time),
                                        ("occupancy", &point.occupancy),
                                    ]),
                                ),
                            ],
                        )
                    })
                    .collect(),
            ),
        ));
    }
    record("Analysis", fields)
}

fn station_analysis(result: &lovelace_systems::AnalysisResult, precision: usize) -> Value {
    let mut fields = provenance_fields(&result.provenance, precision);
    fields.push(("laws", law_records(&result.aggregate.law_checks, precision)));
    if let Some(station) = result.stations.first() {
        fields.extend(station_metrics_fields(station, precision));
    }
    record("StationAnalysis", fields)
}

fn bottleneck_record(report: &lovelace_systems::BottleneckAnalysis, precision: usize) -> Value {
    let stations = Value::Vector(
        report
            .stations
            .iter()
            .map(|entry| {
                let mut fields = vec![
                    ("id", Value::Text(entry.id.clone())),
                    ("service_demand", real(&entry.service_demand, precision)),
                    ("bottleneck", Value::Boolean(entry.bottleneck)),
                ];
                if let Some(servers) = entry.servers {
                    fields.push(("servers", count(servers as u64)));
                }
                if let Some(ceiling) = &entry.ceiling {
                    fields.push(("ceiling", real(ceiling, precision)));
                }
                let mut exact = vec![("service_demand", &entry.service_demand)];
                if let Some(ceiling) = &entry.ceiling {
                    exact.push(("ceiling", ceiling));
                }
                fields.push(("exact_values", exact_values(exact)));
                record("BottleneckStation", fields)
            })
            .collect(),
    );
    let mut fields = vec![
        ("basis", Value::Text(report.basis.as_str().to_string())),
        ("max_demand", real(&report.max_demand, precision)),
        ("stations", stations),
        (
            "warnings",
            Value::Vector(
                report
                    .warnings
                    .iter()
                    .map(|warning| Value::Text(warning.clone()))
                    .collect(),
            ),
        ),
    ];
    match &report.throughput_bound {
        Some(bound) => fields.push(("throughput_bound", real(bound, precision))),
        None => fields.push(("throughput_bound", Value::Void)),
    }
    let mut exact = vec![("max_demand", &report.max_demand)];
    if let Some(bound) = &report.throughput_bound {
        exact.push(("throughput_bound", bound));
    }
    fields.push(("exact_values", exact_values(exact)));
    record("Bottlenecks", fields)
}

fn sweep_record(sweep: &lovelace_systems::SweepResult, precision: usize) -> Value {
    let points = Value::Vector(
        sweep
            .points
            .iter()
            .map(|point| {
                let max_utilization = point
                    .result
                    .stations
                    .iter()
                    .map(|station| station.utilization.clone())
                    .fold(
                        Q::zero(),
                        |acc, value| {
                            if value > acc { value } else { acc }
                        },
                    );
                record(
                    "SweepPoint",
                    vec![
                        ("parameter", real(&point.parameter, precision)),
                        (
                            "throughput",
                            real(&point.result.aggregate.throughput, precision),
                        ),
                        (
                            "response_time",
                            real(&point.result.aggregate.response_time, precision),
                        ),
                        ("max_utilization", real(&max_utilization, precision)),
                        (
                            "exact_values",
                            exact_values(vec![
                                ("parameter", &point.parameter),
                                ("throughput", &point.result.aggregate.throughput),
                                ("response_time", &point.result.aggregate.response_time),
                                ("max_utilization", &max_utilization),
                            ]),
                        ),
                        ("analysis", analysis_record(&point.result, precision)),
                    ],
                )
            })
            .collect(),
    );
    record(
        "Sweep",
        vec![
            ("axis", Value::Text(sweep.axis.as_str().to_string())),
            ("points", points),
            (
                "warnings",
                Value::Vector(
                    sweep
                        .warnings
                        .iter()
                        .map(|warning| Value::Text(warning.clone()))
                        .collect(),
                ),
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use crate::{Engine, LangError};

    fn eval(source: &str) -> String {
        let mut engine = Engine::new();
        let value = engine
            .eval(source)
            .unwrap_or_else(|error| panic!("{source}: {}", error.message));
        engine.format_typed(&value)
    }

    fn eval_ok(source: &str) -> bool {
        let formatted = eval(source);
        assert!(
            formatted == "True (Boolean)",
            "{source} must be True, got {formatted}"
        );
        true
    }

    fn refusal(source: &str) -> LangError {
        let mut engine = Engine::new();
        engine.eval(source).expect_err("the call must be refused")
    }

    #[test]
    fn mm1_metrics_are_exact_rationals() {
        assert!(eval_ok("sys_mm1(8, 10).utilization == 4/5"));
        assert!(eval_ok("sys_mm1(8, 10).response_time == 1/2"));
        assert!(eval_ok("sys_mm1(8, 10).waiting_time == 2/5"));
        assert!(eval_ok("sys_mm1(8, 10).occupancy == 4"));
        assert!(eval_ok("sys_mm1(8, 10).queue_length == 16/5"));
        assert_eq!(eval("sys_mm1(8, 10).exact"), "True (Boolean)");
        assert_eq!(eval("sys_mm1(8, 10).model"), "station-mm1");
    }

    #[test]
    fn mmc_erlang_c_and_infinite_server_are_exact() {
        assert!(eval_ok("sys_mmc(8, 5, 2).probability_waiting == 32/45"));
        assert!(eval_ok("sys_mmc(8, 5, 2).waiting_time == 16/45"));
        assert!(eval_ok("sys_mmc(8, 5, 2).response_time == 5/9"));
        assert!(eval_ok("sys_delay(8, 10).waiting_time == 0"));
        assert!(eval_ok("sys_delay(8, 10).occupancy == 4/5"));
        assert_eq!(eval("sys_delay(8, 10).discipline"), "delay");
    }

    #[test]
    fn station_records_round_trip_into_a_network() {
        assert!(eval_ok("sys_station(\"pool\", 1/10, 1, 4).servers == 4"));
        assert_eq!(
            eval("sys_station(\"io\", 1/10, 1, \"delay\").discipline"),
            "delay"
        );
        assert!(eval_ok(
            "sys_station(\"api\", 1/20, 2).service_demand == 1/10"
        ));
        assert_eq!(
            eval("sys_open(8, [sys_station(\"api\", 1/20, 1)]).workload"),
            "open"
        );
        assert!(eval_ok(
            "sys_closed(2, 9/100, [sys_station(\"cpu\", 1/100, 1)]).population == 2"
        ));
    }

    #[test]
    fn closed_mva_matches_the_golden_recurrence() {
        let model = "sys_closed(2, 9/100, [sys_station(\"cpu\", 1/100, 1)])";
        assert!(eval_ok(&format!("sys_mva({model}).throughput == 2000/101")));
        assert!(eval_ok(&format!(
            "sys_mva({model}).stations[0].response_time == 11/1000"
        )));
        assert!(eval_ok(&format!(
            "sys_mva({model}).stations[0].occupancy == 22/101"
        )));
        assert!(eval_ok(&format!("sys_mva({model}).population == 2")));
        assert_eq!(
            eval(&format!("sys_mva({model}).laws[0].holds")),
            "True (Boolean)"
        );

        let empty = "sys_closed(0, 0, [sys_station(\"cpu\", 1/100, 1)])";
        assert!(eval_ok(&format!("sys_mva({empty}).throughput == 0")));
        assert_eq!(eval(&format!("sys_mva({empty}).status")), "degenerate");
    }

    #[test]
    fn open_jackson_and_bottleneck_agree_with_the_hand_computation() {
        let model = "sys_open(8, [sys_station(\"api\", 1/20, 1), sys_station(\"db\", 1/50, 2)])";
        assert!(eval_ok(&format!(
            "sys_analyze({model}).response_time == 29/204"
        )));
        assert!(eval_ok(&format!(
            "sys_analyze({model}).stations[1].throughput == 16"
        )));
        assert_eq!(
            eval(&format!("sys_analyze({model}).laws[0].holds")),
            "True (Boolean)"
        );

        let pool = "sys_open(8, [sys_station(\"api\", 1/20, 1), sys_station(\"db\", 1/50, 3)])";
        assert!(eval_ok(&format!(
            "sys_bottlenecks({pool}).throughput_bound == 50/3"
        )));
        assert_eq!(
            eval(&format!("sys_bottlenecks({pool}).stations[1].bottleneck")),
            "True (Boolean)"
        );
    }

    #[test]
    fn sweeps_are_ordered_batches() {
        let sweep = "sys_sweep(sys_closed(1, 0, [sys_station(\"cpu\", 1/10, 1)]), 1, 3, 1)";
        assert_eq!(eval(&format!("{sweep}.axis")), "population");
        assert!(eval_ok(&format!("{sweep}.points[2].parameter == 3")));
        assert!(eval_ok(&format!("{sweep}.points[0].throughput == 10")));
        assert!(eval_ok(&format!(
            "{sweep}.points[1].analysis.throughput == 10"
        )));
    }

    #[test]
    fn unstable_and_unsupported_models_are_refused_with_named_codes() {
        let unstable = refusal("sys_mm1(10, 10)");
        assert_eq!(unstable.code, "UnstableStation");
        assert_eq!(unstable.category, "DomainError");

        let unsupported = refusal("sys_mva(sys_open(1, [sys_station(\"api\", 1, 1)]))");
        assert_eq!(unsupported.code, "UnsupportedModel");
        assert_eq!(unsupported.category, "UnsupportedOperation");

        let wrong_shape = refusal("sys_mm1([1, 2], 3)");
        assert_eq!(wrong_shape.code, "InvalidArgument");
        assert_eq!(wrong_shape.category, "TypeMismatch");
        assert_eq!(
            wrong_shape.message,
            "sys_mm1(): argument 1 expects a number; got Vector."
        );

        let bad_id = refusal("sys_station(\"has space\", 1, 1)");
        assert_eq!(bad_id.code, "InvalidIdentifier");
        assert_eq!(bad_id.category, "DomainError");
    }

    #[test]
    fn exact_values_cross_the_language_boundary_beside_their_decimal_projection() {
        // The decimal field stays a projection...
        assert_eq!(eval("sys_mm1(8, 10).utilization"), "0.8 (Real)");
        // ...and the exact sidecar carries the canonical numerator/denominator pair.
        assert_eq!(
            eval("sys_mm1(8, 10).exact_values.utilization.numerator"),
            "4 (Integer)"
        );
        assert_eq!(
            eval("sys_mm1(8, 10).exact_values.utilization.denominator"),
            "5 (Natural)"
        );
        assert_eq!(
            eval("sys_mm1(8, 10).exact_values.response_time.numerator"),
            "1 (Integer)"
        );
        assert_eq!(
            eval("sys_mm1(8, 10).exact_values.response_time.denominator"),
            "2 (Natural)"
        );

        // A periodic exact value keeps its period in the projection and its fraction in the
        // sidecar; the projection alone is not treated as the exact value.
        assert_eq!(eval("sys_mm1(1, 3).utilization"), "0.(3) (Real)");
        assert_eq!(
            eval("sys_mm1(1, 3).exact_values.utilization.denominator"),
            "3 (Natural)"
        );

        let model = "sys_closed(2, 9/100, [sys_station(\"cpu\", 1/100, 1)])";
        assert_eq!(
            eval(&format!(
                "sys_mva({model}).exact_values.throughput.numerator"
            )),
            "2000 (Integer)"
        );
        assert_eq!(
            eval(&format!(
                "sys_mva({model}).exact_values.throughput.denominator"
            )),
            "101 (Natural)"
        );
        assert_eq!(
            eval(&format!(
                "sys_mva({model}).stations[0].exact_values.occupancy.numerator"
            )),
            "22 (Integer)"
        );
        assert_eq!(
            eval(&format!(
                "sys_mva({model}).laws[0].exact_values.left.numerator"
            )),
            "2000 (Integer)"
        );

        let sweep = "sys_sweep(sys_closed(1, 0, [sys_station(\"cpu\", 1/10, 1)]), 1, 3, 1)";
        assert_eq!(
            eval(&format!(
                "{sweep}.points[0].exact_values.throughput.denominator"
            )),
            "1 (Natural)"
        );
    }

    #[test]
    fn a_precision_limited_real_is_refused_as_an_exact_model_input() {
        // `pi(30)` is a truncated Real. Promoting its stored digits to an exact rational would
        // silently restore precision that was never stored (RNL-NUM-009), so the adapter refuses.
        let refused = refusal("sys_mm1(pi(30), 10)");
        assert_eq!(refused.code, "InvalidArgument");
        assert_eq!(refused.category, "TypeMismatch");
        assert!(
            refused.message.contains("precision-limited"),
            "{}",
            refused.message
        );

        // An exact periodic Real (1/3) is lossless and stays accepted.
        assert!(eval_ok("sys_mm1(1/3, 10).utilization == 1/30"));
    }

    #[test]
    fn the_same_source_is_deterministic() {
        let source = "sys_analyze(sys_open(8, [sys_station(\"api\", 1/20, 1)]))";
        assert_eq!(eval(source), eval(source));
    }
}
