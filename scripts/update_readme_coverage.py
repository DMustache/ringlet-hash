#!/usr/bin/env python3
import argparse
import json
from pathlib import Path
from urllib.parse import quote


def percentage(covered: int, total: int) -> str:
    if total == 0:
        raise ValueError("coverage report has no measurable items")
    return f"{covered / total * 100:.1f}".rstrip("0").rstrip(".")


def color(covered: int, total: int) -> str:
    ratio = covered / total
    if ratio >= 0.9:
        return "brightgreen"
    if ratio >= 0.8:
        return "green"
    if ratio >= 0.7:
        return "yellow"
    return "red"


def badge(label: str, covered: int, total: int) -> str:
    value = f"{percentage(covered, total)}%"
    url = f"https://img.shields.io/badge/{quote(label, safe='')}-{quote(value, safe='')}-{color(covered, total)}"
    return f"![{label}: {value}]({url})"


def replace_block(source: str, name: str, content: str) -> str:
    start = f"<!-- {name}:start -->"
    end = f"<!-- {name}:end -->"
    if source.count(start) != 1 or source.count(end) != 1:
        raise ValueError(f"README must contain exactly one {name} marker pair")
    before, remainder = source.split(start, 1)
    _, after = remainder.split(end, 1)
    return f"{before}{start}\n{content}\n{end}{after}"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("coverage_json", type=Path)
    parser.add_argument("docs_json", type=Path)
    parser.add_argument("readme", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    coverage = json.loads(args.coverage_json.read_text())
    lines = coverage["data"][0]["totals"]["lines"]
    test_covered, test_total = lines["covered"], lines["count"]

    docs = json.loads(args.docs_json.read_text())
    doc_covered = sum(file["with_docs"] for file in docs.values())
    doc_total = sum(file["total"] for file in docs.values())

    for covered, total in ((test_covered, test_total), (doc_covered, doc_total)):
        if not 0 <= covered <= total:
            raise ValueError("invalid coverage counts")

    badges = "\n".join(
        (
            badge("Test line coverage", test_covered, test_total),
            badge("Public API docs", doc_covered, doc_total),
        )
    )
    details = "\n".join(
        (
            "| Metric | Result | How it is measured |",
            "| --- | ---: | --- |",
            f"| Test line coverage | {percentage(test_covered, test_total)}% ({test_covered}/{test_total} lines) | `cargo llvm-cov` on library source; tests and benchmarks excluded |",
            f"| Public API documentation | {percentage(doc_covered, doc_total)}% ({doc_covered}/{doc_total} items) | Rustdoc `--show-coverage` documented-item count |",
        )
    )

    readme = args.readme.read_text()
    readme = replace_block(readme, "coverage-badges", badges)
    readme = replace_block(readme, "coverage-details", details)
    args.output.write_text(readme)


if __name__ == "__main__":
    main()
