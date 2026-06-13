#!/usr/bin/env python3
"""Verify a Maelstrom run by parsing store/<workload>/latest/results.edn.

EDN is read with regex rather than a parser — we only need a few well-known
scalar keys (:valid?, :msgs-per-op, :latency quantiles), and the relevant
Maelstrom outputs are stable across releases for grep-shaped extraction.

All requested rules are evaluated. One line is printed per rule with its
status, actual value, and target. Exit code is 0 iff every rule passed.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
STORE = REPO_ROOT / "maelstrom" / "store"

NOT_FOUND = "<not found>"


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser()
    p.add_argument("--workload", required=True)
    p.add_argument("--max-msgs-per-op", type=float, default=None)
    p.add_argument("--max-median-latency-ms", type=float, default=None)
    p.add_argument("--max-latency-ms", type=float, default=None)
    return p.parse_args()


def find_results(workload: str) -> Path:
    latest = STORE / workload / "latest"
    if not latest.exists():
        sys.exit(f"FAIL: no run at {latest}")
    return (latest / "results.edn").resolve()


def get_valid(text: str) -> str | None:
    # Top-level :valid? is the last :valid? in the file.
    matches = re.findall(r":valid\?\s+(true|false|:unknown)", text)
    return matches[-1] if matches else None


def get_servers_msgs_per_op(text: str) -> float | None:
    # Prefer :servers block (server-to-server traffic); fall back to first match.
    m = re.search(r":servers\s*\{[^{}]*?:msgs-per-op\s+([\d.]+)", text)
    if m:
        return float(m.group(1))
    m = re.search(r":msgs-per-op\s+([\d.]+)", text)
    return float(m.group(1)) if m else None


def get_latency_quantiles(text: str) -> tuple[dict[float, float], str] | None:
    # Maps look like {0 .., 0.5 .., 0.95 .., 0.99 .., 1 ..}. Broadcast's
    # :stable-latencies values are already in milliseconds; Jepsen's
    # classic :latency map (other workloads) is in nanoseconds.
    m = re.search(r":stable-latencies\s*\{([^{}]+)\}", text)
    if m:
        pairs = re.findall(r"([\d.]+)\s+([\d.]+)", m.group(1))
        return {float(k): float(v) for k, v in pairs}, "ms"
    # (?<!-) avoids matching `-latency` substrings like `:stable-latencies`.
    m = re.search(r"(?<!-):latency\s*\{([^{}]+)\}", text)
    if m:
        pairs = re.findall(r"([\d.]+)\s+([\d.]+)", m.group(1))
        return {float(k): float(v) for k, v in pairs}, "ns"
    return None


def load_latency(
    results: Path, text: str
) -> tuple[dict[float, float], str] | None:
    found = get_latency_quantiles(text)
    if found is not None:
        return found
    log = results.parent / "jepsen.log"
    if log.exists():
        return get_latency_quantiles(log.read_text(errors="ignore"))
    return None


@dataclass
class Rule:
    name: str
    actual: str
    target: str
    passed: bool


def evaluate(args: argparse.Namespace, results: Path, text: str) -> list[Rule]:
    rules: list[Rule] = []

    valid = get_valid(text)
    rules.append(
        Rule(
            name="valid?",
            actual=valid if valid is not None else NOT_FOUND,
            target="true",
            passed=valid == "true",
        )
    )

    if args.max_msgs_per_op is not None:
        mpo = get_servers_msgs_per_op(text)
        rules.append(
            Rule(
                name="msgs-per-op",
                actual=f"{mpo:.2f}" if mpo is not None else NOT_FOUND,
                target=f"<={args.max_msgs_per_op:g}",
                passed=mpo is not None and mpo <= args.max_msgs_per_op,
            )
        )

    if args.max_median_latency_ms is not None or args.max_latency_ms is not None:
        found = load_latency(results, text)
        block, unit = (found if found is not None else (None, None))
        factor = 1.0 if unit == "ms" else (1.0 / 1_000_000.0)

        def to_ms(raw: float | None) -> float | None:
            return raw * factor if raw is not None else None

        if args.max_median_latency_ms is not None:
            median_ms = to_ms(block.get(0.5)) if block is not None else None
            rules.append(
                Rule(
                    name="median-latency",
                    actual=f"{median_ms:.0f}ms" if median_ms is not None else NOT_FOUND,
                    target=f"<={args.max_median_latency_ms:g}ms",
                    passed=median_ms is not None
                    and median_ms <= args.max_median_latency_ms,
                )
            )
        if args.max_latency_ms is not None:
            raw = block.get(1, block.get(1.0)) if block is not None else None
            max_ms = to_ms(raw)
            rules.append(
                Rule(
                    name="max-latency",
                    actual=f"{max_ms:.0f}ms" if max_ms is not None else NOT_FOUND,
                    target=f"<={args.max_latency_ms:g}ms",
                    passed=max_ms is not None and max_ms <= args.max_latency_ms,
                )
            )

    return rules


def render(rules: list[Rule]) -> str:
    name_w = max(len(r.name) for r in rules)
    actual_w = max(len(r.actual) for r in rules)
    lines = []
    for r in rules:
        status = "PASS" if r.passed else "FAIL"
        lines.append(
            f"{status}  {r.name:<{name_w}}  actual={r.actual:<{actual_w}}  target={r.target}"
        )
    return "\n".join(lines)


def main() -> None:
    args = parse_args()
    results = find_results(args.workload)
    text = results.read_text()
    rules = evaluate(args, results, text)
    print(render(rules))
    if not all(r.passed for r in rules):
        sys.exit(1)


if __name__ == "__main__":
    main()
