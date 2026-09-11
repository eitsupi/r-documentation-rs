#!/usr/bin/env python3
"""Compare the independent Rust scan and R oracle reports."""

from __future__ import annotations

import json
import sys
from pathlib import Path


def fail(message: str) -> None:
    raise ValueError(message)


def read_json_lines(path: Path) -> list[dict]:
    records: list[dict] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as error:
            fail(f"{path}:{line_number}: invalid JSON: {error}")
        if not isinstance(value, dict):
            fail(f"{path}:{line_number}: report record is not an object")
        records.append(value)
    return records


def manifest_packages(path: Path, profile: str) -> set[str]:
    packages: set[str] = set()
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 9:
            fail(f"{path}:{line_number}: expected 9 tab-separated fields")
        if fields[0] == profile:
            if fields[1] in packages:
                fail(f"manifest repeats package {fields[1]!r}")
            packages.add(fields[1])
    if not packages:
        fail(f"manifest has no packages for {profile!r}")
    return packages


def package_map(records: list[dict], source: Path, expected: set[str], profile: str) -> dict[str, dict]:
    result: dict[str, dict] = {}
    for record in records:
        package = record.get("package")
        if not isinstance(package, str) or package not in expected:
            fail(f"{source}: package is outside the manifest: {package!r}")
        if package in result:
            fail(f"{source}: duplicate package report for {package!r}")
        if record.get("profile") != profile:
            fail(f"{source}: package {package!r} has the wrong profile")
        if record.get("schema") != "installed-code-corpus/v1":
            fail(f"{source}: package {package!r} has an unknown schema")
        result[package] = record
    missing = expected - result.keys()
    if missing:
        fail(f"{source}: missing report for package(s): {sorted(missing)}")
    return result


def names_from_entries(entries: list[dict]) -> list[str]:
    names: list[str] = []
    for entry in entries:
        name = entry.get("name")
        if not isinstance(name, str):
            fail("stored entry has no string name")
        names.append(name)
    return names


def validate_entries(package: str, source: Path, entries: list[dict]) -> list[str]:
    if not isinstance(entries, list):
        fail(f"{source}: {package}: stored_entries is not an array")
    record_ids: set[tuple[str, int]] = set()
    allowed_fetch = {"ok", "ambiguous", "record_error", "prefix_error"}
    allowed_error_reasons = {
        "io", "compression", "range", "stored_size", "decompressed_size",
        "decompression", "framing", "reference", "database_changed", "other",
    }
    allowed_formals = {"available", "not_applicable", "unavailable", "not_observed"}
    for position, entry in enumerate(entries):
        index = entry.get("index")
        if not isinstance(index, int) or index != position:
            fail(f"{source}: {package}: invalid stored record index")
        record_id = (package, index)
        if record_id in record_ids:
            fail(f"{source}: duplicate record ID {record_id!r}")
        record_ids.add(record_id)
        outcome = entry.get("record_fetch")
        if outcome not in allowed_fetch:
            fail(f"{source}: {package}: unknown record outcome {outcome!r}")
        if outcome == "ok" and (
            not isinstance(entry.get("root_kind"), str)
            or not isinstance(entry.get("type_code"), int)
        ):
            fail(f"{source}: {package}: successful record has no root-kind observation")
        if outcome == "record_error":
            reason = entry.get("record_error_reason")
            if reason not in allowed_error_reasons:
                fail(f"{source}: {package}: record error has no classified reason")
        if entry.get("root_kind") == "Unknown":
            fail(f"{source}: {package}: unknown root-kind bucket")
        if entry.get("formals_state") not in allowed_formals:
            fail(f"{source}: {package}: unknown formals bucket")
        if entry.get("formals_state") == "unavailable" and entry.get("formals_reason") in (None, "unknown"):
            fail(f"{source}: {package}: unavailable formals has no classified reason")
        if entry.get("formals_state") == "not_applicable" and entry.get("formals_reason") in (None, "unknown"):
            fail(f"{source}: {package}: not-applicable formals has no classified reason")
        if "runtime_formals_state" in entry and entry["runtime_formals_state"] not in allowed_formals:
            fail(f"{source}: {package}: unknown runtime formals bucket")
        if source.name == "r-oracle" and any(
            field not in entry for field in ("runtime_formals_state", "runtime_formals", "runtime_formals_reason")
        ):
            fail(f"{source}: {package}: runtime formals observation is missing")
        if source.name == "r-oracle" and "oracle_eligible" not in entry:
            fail(f"{source}: {package}: oracle_eligible is missing")
        if "oracle_eligible" in entry and not isinstance(entry["oracle_eligible"], bool):
            fail(f"{source}: {package}: oracle_eligible is not boolean")
    return names_from_entries(entries)


def compare_entries(package: str, scanner: dict, oracle: dict, mismatches: list[str]) -> int:
    scanner_entries = scanner.get("stored_entries")
    oracle_entries = oracle.get("stored_entries")
    scanner_names = validate_entries(package, Path("rust-scanner"), scanner_entries)
    oracle_names = validate_entries(package, Path("r-oracle"), oracle_entries)
    if scanner_names != oracle_names:
        mismatches.append("stored entry order or names differ")
        return 0
    runtime_kinds = {
        item["name"]: item.get("kind")
        for item in oracle.get("runtime_kinds", [])
        if isinstance(item, dict) and isinstance(item.get("name"), str)
    }
    active = set(oracle.get("active_bindings", []))
    runtime_names = set(oracle.get("runtime_names", []))
    compared = 0
    for scanner_entry, oracle_entry in zip(scanner_entries, oracle_entries):
        name = scanner_entry["name"]
        eligible = (
            scanner_entry.get("record_fetch") == "ok"
            and scanner_entry.get("root_kind") == "Closure"
            and name in runtime_names
            and name not in active
            and runtime_kinds.get(name) == "Closure"
        )
        if oracle_entry.get("oracle_eligible") != eligible:
            mismatches.append(
                f"{package}[{scanner_entry['index']}].oracle_eligible: "
                f"derived={eligible!r}, reported={oracle_entry.get('oracle_eligible')!r}"
            )
        # A closure can be eligible for the oracle while the bounded Rust
        # inspector deliberately cannot obtain its formals (for example, an
        # unsupported attribute). Keep that safety boundary in the aggregate
        # and compare only observations available from both processes.
        comparable = (
            eligible
            and scanner_entry.get("formals_state") == "available"
            and oracle_entry.get("runtime_formals_state") == "available"
        )
        if comparable:
            compared += 1
        fields = ["record_fetch"]
        if comparable:
            if scanner_entry.get("formals_state") != oracle_entry.get("runtime_formals_state"):
                mismatches.append(
                    f"{package}[{scanner_entry['index']}].formals_state: "
                    f"scanner={scanner_entry.get('formals_state')!r}, "
                    f"oracle={oracle_entry.get('runtime_formals_state')!r}"
                )
            elif scanner_entry.get("formals") != oracle_entry.get("runtime_formals"):
                mismatches.append(
                    f"{package}[{scanner_entry['index']}].formals: "
                    f"scanner={scanner_entry.get('formals')!r}, "
                    f"oracle={oracle_entry.get('runtime_formals')!r}"
                )
        if (
            scanner_entry.get("record_fetch") == "ok"
            and oracle_entry.get("record_fetch") == "ok"
            and root_kind_is_comparable(scanner_entry)
        ):
            for field in ("root_kind", "type_code"):
                if scanner_entry.get(field) != oracle_entry.get(field):
                    mismatches.append(
                        f"{package}[{scanner_entry['index']}].{field}: "
                        f"scanner={scanner_entry.get(field)!r}, oracle={oracle_entry.get(field)!r}"
                    )
        for field in fields:
            if scanner_entry.get(field) != oracle_entry.get(field):
                mismatches.append(
                    f"{package}[{scanner_entry['index']}].{field}: "
                    f"scanner={scanner_entry.get(field)!r}, oracle={oracle_entry.get(field)!r}"
                )
    return compared


def sorted_names(record: dict, field: str) -> list[str]:
    values = record.get(field)
    if not isinstance(values, list) or not all(isinstance(value, str) for value in values):
        fail(f"{record.get('package')}: {field} is not a string array")
    return sorted(set(values))


def root_kind_is_comparable(entry: dict) -> bool:
    """Return whether the independent R report has a portable root-kind tag."""
    code = entry.get("type_code")
    return isinstance(code, int) and code in {
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 13, 14, 15, 16, 17, 19, 21, 22, 23, 24, 25,
        247, 248, 249, 255,
    }


def record_outcome_counts(entries: list[dict]) -> dict[str, int]:
    counts = {"ok": 0, "ambiguous": 0, "record_error": 0, "prefix_error": 0}
    for entry in entries:
        outcome = entry["record_fetch"]
        counts[outcome] += 1
    return counts


def root_kind_counts(entries: list[dict]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for entry in entries:
        kind = entry.get("root_kind")
        if kind is None:
            continue
        if not isinstance(kind, str) or kind == "Unknown":
            fail("unknown root-kind bucket")
        counts[kind] = counts.get(kind, 0) + 1
    return dict(sorted(counts.items()))


def formals_counts(entries: list[dict]) -> dict:
    available = 0
    not_applicable = 0
    not_observed = 0
    unavailable: dict[str, int] = {}
    for entry in entries:
        state = entry.get("formals_state")
        if state == "available":
            available += 1
        elif state == "not_applicable":
            not_applicable += 1
        elif state == "not_observed":
            not_observed += 1
        elif state == "unavailable":
            reason = entry.get("formals_reason")
            phase = entry.get("formals_phase")
            type_code = entry.get("type_code")
            key = f"{reason or 'unknown'}@{phase or 'none'}@{type_code if type_code is not None else 'none'}"
            unavailable[key] = unavailable.get(key, 0) + 1
        else:
            fail(f"unknown formals bucket {state!r}")
    return {
        "available": available,
        "not_applicable": not_applicable,
        "not_observed": not_observed,
        "unavailable": dict(sorted(unavailable.items())),
    }


def main() -> int:
    if len(sys.argv) != 7:
        print("usage: compare_installed_code_corpus.py PROFILE MANIFEST EXPECTED RUST_REPORT R_ORACLE_REPORT SUMMARY", file=sys.stderr)
        return 2
    profile, manifest_name, expected_name, scanner_name, oracle_name, summary_name = sys.argv[1:]
    manifest = Path(manifest_name)
    expected_path = None if expected_name == "-" else Path(expected_name)
    scanner_path = Path(scanner_name)
    oracle_path = Path(oracle_name)
    expected = manifest_packages(manifest, profile)
    expected_packages = None
    if expected_path is not None:
        expected_report = json.loads(expected_path.read_text(encoding="utf-8"))
        if expected_report.get("schema") != "installed-code-corpus-expected/v1":
            fail(f"{expected_path}: unknown expected-report schema")
        if expected_report.get("profile") != profile:
            fail(f"{expected_path}: expected-report profile mismatch")
        expected_packages = expected_report.get("packages")
        if not isinstance(expected_packages, dict) or set(expected_packages) != expected:
            fail(f"{expected_path}: expected package set differs from manifest")
    scanner = package_map(read_json_lines(scanner_path), scanner_path, expected, profile)
    oracle = package_map(read_json_lines(oracle_path), oracle_path, expected, profile)

    package_summaries: list[dict] = []
    total_compared = 0
    all_mismatches: list[str] = []
    for package in sorted(expected):
        scanner_record = scanner[package]
        oracle_record = oracle[package]
        baseline = expected_packages.get(package) if expected_packages is not None else None
        if baseline is not None:
            if scanner_record.get("package_version") != baseline.get("package_version"):
                all_mismatches.append(f"{package}: scanner package version drifted from expected report")
            if oracle_record.get("package_version") != baseline.get("package_version"):
                all_mismatches.append(f"{package}: oracle package version drifted from expected report")
            for field in ("stored_entry_count", "root_kind_counts", "formals", "oracle"):
                if field not in baseline:
                    fail(f"{expected_path}: {package}: expected {field} is not fixed")
        mismatches: list[str] = []
        compared = compare_entries(package, scanner_record, oracle_record, mismatches)
        if baseline is not None and len(scanner_record.get("stored_entries", [])) != baseline["stored_entry_count"]:
            mismatches.append(f"{package}: stored entry count differs from expected report")
        total_compared += compared
        scanner_entries = scanner_record["stored_entries"]
        unique_stored = set(scanner_record.get("unique_names", []))
        declared = set(scanner_record.get("declared_names", []))
        runtime = set(oracle_record.get("runtime_names", []))
        runtime_kinds = {
            item["name"]: item.get("kind")
            for item in oracle_record.get("runtime_kinds", [])
            if isinstance(item, dict) and isinstance(item.get("name"), str)
        }
        root_kinds = {
            entry["name"]: entry.get("root_kind")
            for entry in scanner_entries
            if entry.get("record_fetch") == "ok" and entry.get("name") in unique_stored
        }
        runtime_kind_mismatches = sorted(
            name for name, kind in root_kinds.items()
            if name in runtime_kinds and runtime_kinds[name] != kind
        )
        eligible_count = sum(
            entry.get("record_fetch") == "ok"
            and entry.get("root_kind") == "Closure"
            and entry.get("name") in runtime
            and entry.get("name") not in set(oracle_record.get("active_bindings", []))
            and runtime_kinds.get(entry.get("name")) == "Closure"
            for entry in scanner_entries
        )
        runtime_missing = sorted(unique_stored - runtime)
        root_comparable = [
            entry
            for entry, oracle_entry in zip(scanner_entries, oracle_record["stored_entries"])
            if entry.get("record_fetch") == "ok"
            and oracle_entry.get("record_fetch") == "ok"
            and root_kind_is_comparable(entry)
        ]
        root_kind_mismatches = [
            entry["index"]
            for entry in root_comparable
            if entry.get("root_kind") != oracle_record["stored_entries"][entry["index"]].get("root_kind")
            or entry.get("type_code") != oracle_record["stored_entries"][entry["index"]].get("type_code")
        ]
        set_diff = {
            "stored_not_declared": sorted(unique_stored - declared),
            "declared_not_stored": sorted(declared - unique_stored),
            "runtime_not_declared": sorted(runtime - declared),
        }
        package_summary = {
            "package": package,
            "stored_entry_count": len(scanner_entries),
            "record_outcome_counts": record_outcome_counts(scanner_entries),
            "root_kind_counts": root_kind_counts(scanner_entries),
            "formals": formals_counts(scanner_entries),
            "oracle_eligibility": {
                "eligible": eligible_count,
                "ineligible": len(scanner_entries) - eligible_count,
            },
            "oracle_compared": compared,
            "oracle_mismatches": mismatches,
            "runtime_missing": runtime_missing,
            "active_bindings": sorted_names(oracle_record, "active_bindings"),
            "runtime_kind_mismatches": runtime_kind_mismatches,
            "stored_root_kind_comparison": {
                "comparable": len(root_comparable),
                "mismatch_count": len(root_kind_mismatches),
                "mismatches": root_kind_mismatches,
            },
            "stored_declared_runtime_set_diff": set_diff,
        }
        package_summaries.append(package_summary)
        all_mismatches.extend(mismatches)

        if baseline is not None:
            observed = {
                "stored_entry_count": package_summary["stored_entry_count"],
                "record_outcome_counts": package_summary["record_outcome_counts"],
                "root_kind_counts": package_summary["root_kind_counts"],
                "formals": package_summary["formals"],
                "oracle": {
                    "eligible": eligible_count,
                    "compared": compared,
                    "mismatches": len(mismatches),
                    "root_kind_comparable": package_summary["stored_root_kind_comparison"]["comparable"],
                    "root_kind_mismatches": package_summary["stored_root_kind_comparison"]["mismatch_count"],
                },
            }
            for field, value in observed.items():
                if baseline.get(field) != value:
                    all_mismatches.append(
                        f"{package}: expected {field} drifted: observed={value!r}, expected={baseline.get(field)!r}"
                    )

    summary = {
        "schema": "installed-code-corpus-summary/v1",
        "profile": profile,
        "packages": package_summaries,
        "oracle_eligibility": {
            "eligible": sum(item["oracle_eligibility"]["eligible"] for item in package_summaries),
            "ineligible": sum(item["oracle_eligibility"]["ineligible"] for item in package_summaries),
        },
        "oracle_compared": total_compared,
        "oracle_mismatch": all_mismatches,
    }
    if expected_path is not None:
        totals = expected_report.get("totals")
        if not isinstance(totals, dict):
            fail(f"{expected_path}: expected totals are not fixed")
        observed_totals = {
            "stored_entry_count": sum(item["stored_entry_count"] for item in package_summaries),
            "record_outcome_counts": {
                outcome: sum(item["record_outcome_counts"][outcome] for item in package_summaries)
                for outcome in ("ok", "ambiguous", "record_error", "prefix_error")
            },
            "oracle_eligible": sum(item["oracle_eligibility"]["eligible"] for item in package_summaries),
            "oracle_compared": total_compared,
            "oracle_mismatches": len(all_mismatches),
        }
        if totals != observed_totals:
            all_mismatches.append(
                f"expected totals drifted: observed={observed_totals!r}, expected={totals!r}"
            )
        summary["expected_totals"] = totals
    Path(summary_name).write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if all_mismatches:
        print("installed-code corpus mismatch:", file=sys.stderr)
        for mismatch in all_mismatches:
            print(f"  {mismatch}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
