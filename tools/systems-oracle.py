#!/usr/bin/env python3
"""Independent systems-result oracle for RustLace (RNL-CI-006).

The formulas below are written from their textbook definitions against Python's
`fractions.Fraction`; they do not call, import or mirror RustLace's production solver code. The
fixture they emit (`parity/systems/oracle.tsv`) is consumed by
`lovelace-systems/tests/oracle.rs`.

Cases:
  mm1_8_10        M/M/1 with lambda=8, mu=10
  mmc_8_5_2       M/M/c with lambda=8, mu=5, c=2 (Erlang C)
  mva_closed      single-class MVA, N=4, Z=1/10, D = 1/100, 1/25, 1/500 (delay)
  jackson_open    open Jackson, lambda=20, four stations with visits

Usage:
    python tools/systems-oracle.py --check
    python tools/systems-oracle.py --write
"""

from __future__ import annotations

import sys
from fractions import Fraction
from pathlib import Path

FIXTURE = Path("parity/systems/oracle.tsv")


def text(value: Fraction) -> str:
    return f"{value.numerator}/{value.denominator}"


def erlang_c(traffic: Fraction, servers: int) -> Fraction:
    """The Erlang C waiting probability from its independent series definition."""
    partial = sum(traffic**k / factorial(k) for k in range(servers))
    tail = traffic**servers / (factorial(servers) * (1 - traffic / servers))
    return tail / (partial + tail)


def factorial(n: int) -> int:
    value = 1
    for k in range(2, n + 1):
        value *= k
    return value


def mm1(lam: Fraction, mu: Fraction) -> dict[str, Fraction]:
    rho = lam / mu
    response = 1 / (mu - lam)
    wait = rho * response
    return {
        "utilization": rho,
        "response_time": response,
        "waiting_time": wait,
        "occupancy": lam * response,
        "queue_length": lam * wait,
        "throughput": lam,
    }


def mmc(lam: Fraction, mu: Fraction, servers: int) -> dict[str, Fraction]:
    service_time = 1 / mu
    traffic = lam * service_time
    probability = erlang_c(traffic, servers)
    spare = servers * mu - lam
    wait = probability / spare
    response = wait + service_time
    return {
        "probability_waiting": probability,
        "waiting_time": wait,
        "response_time": response,
        "utilization": lam / (servers * mu),
        "occupancy": lam * response,
        "queue_length": lam * wait,
        "throughput": lam,
    }


def mva_closed(
    population: int, think: Fraction, demands: list[Fraction], delay: list[bool]
) -> dict[str, Fraction]:
    """Single-class MVA written directly from the arrival-theorem recurrence."""
    queues = [Fraction(0) for _ in demands]
    throughput = Fraction(0)
    response = Fraction(0)
    for n in range(1, population + 1):
        residences = [
            demand if is_delay else demand * (1 + queues[index])
            for index, (demand, is_delay) in enumerate(zip(demands, delay))
        ]
        cycle = think + sum(residences)
        throughput = Fraction(n) / cycle
        queues = [throughput * value for value in residences]
        response = sum(residences)
    return {
        "throughput": throughput,
        "response_time": response,
        "occupancy": sum(queues),
        "occupancy_station_0": queues[0],
        "occupancy_station_1": queues[1],
        "occupancy_station_2": queues[2],
    }


def jackson_open(lam: Fraction) -> dict[str, Fraction]:
    """Each station is an independent M/M/1 (or delay center) at its visit rate."""
    stations = [
        ("api", Fraction(1, 50), Fraction(1), False),
        ("db", Fraction(1, 100), Fraction(2), False),
        ("search", Fraction(1, 200), Fraction(3), False),
        ("cache", Fraction(1, 20), Fraction(1), True),
    ]
    response = Fraction(0)
    occupancy = Fraction(0)
    queue = Fraction(0)
    utilization: dict[str, Fraction] = {}
    for name, service_time, visits, is_delay in stations:
        demand = visits * service_time
        arrival = lam * visits
        if is_delay:
            station_response = service_time
            station_occupancy = arrival * station_response
            station_queue = Fraction(0)
            utilization[name] = station_occupancy
        else:
            rho = arrival * service_time
            if rho >= 1:
                raise AssertionError(f"unstable oracle case: {name}")
            station_response = service_time / (1 - rho)
            station_occupancy = arrival * station_response
            station_queue = arrival * (rho * station_response)
            utilization[name] = rho
        response += visits * station_response
        occupancy += station_occupancy
        queue += station_queue
    return {
        "response_time": response,
        "occupancy": occupancy,
        "queue_length": queue,
        "utilization_api": utilization["api"],
        "utilization_db": utilization["db"],
        "utilization_search": utilization["search"],
    }


def cases() -> list[tuple[str, str, Fraction]]:
    out: list[tuple[str, str, Fraction]] = []

    def emit(case: str, metrics: dict[str, Fraction]) -> None:
        for name in sorted(metrics):
            out.append((case, name, metrics[name]))

    emit("mm1_8_10", mm1(Fraction(8), Fraction(10)))
    emit("mmc_8_5_2", mmc(Fraction(8), Fraction(5), 2))
    emit(
        "mva_closed",
        mva_closed(
            4,
            Fraction(1, 10),
            [Fraction(1, 100), Fraction(1, 25), Fraction(1, 500)],
            [False, False, True],
        ),
    )
    emit("jackson_open", jackson_open(Fraction(20)))
    return out


def render() -> str:
    lines = [
        "# RustLace systems oracle (generated by tools/systems-oracle.py).",
        "# Source of truth: textbook formulas over Python fractions.Fraction, no Rust code.",
        "# Format: case<TAB>metric<TAB>canonical rational.",
    ]
    for case, metric, value in cases():
        lines.append(f"{case}\t{metric}\t{text(value)}")
    return "\n".join(lines) + "\n"


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    target = root / FIXTURE
    generated = render()
    if "--write" in sys.argv:
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(generated, encoding="utf-8", newline="\n")
        print(f"wrote {target.relative_to(root)} ({len(cases())} metrics)")
        return 0
    if "--check" in sys.argv:
        if not target.exists():
            print(f"missing oracle fixture: {target}", file=sys.stderr)
            return 2
        current = target.read_text(encoding="utf-8")
        if current != generated:
            print(
                "systems oracle fixture is stale; run python tools/systems-oracle.py --write",
                file=sys.stderr,
            )
            return 1
        print(f"systems oracle: {len(cases())} metrics match the independent formulas")
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
