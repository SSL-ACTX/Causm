#!/usr/bin/env python3
"""
python_telemetry_linter.py
Test Causm IPC (JSON-RPC stdio) plugin.
Performs semantic AST auditing:
- Telemetry & logging routine naming policies
- WCET budget analysis (flags missing/zero budgets)
- Identifies unhandled decayed entropic patterns in match blocks
- Recursively audits isolate statements and capability boundaries
"""

import json
import sys
from typing import Any


class CausmAstLinter:
    def __init__(self, request: dict[str, Any]) -> None:
        self.request = request
        self.ast = request.get("ast", {})
        self.file_path = request.get("file_path", "")
        self.options = request.get("options", {})
        self.strict_naming = (
            str(self.options.get("strict_naming", "true")).lower() == "true"
        )
        self.require_wcet = (
            str(self.options.get("require_wcet", "true")).lower() == "true"
        )
        self.diagnostics: list[dict[str, Any]] = []

    def add_diagnostic(
        self,
        level: str,
        message: str,
        span: dict[str, Any] | None = None,
    ) -> None:
        self.diagnostics.append(
            {
                "level": level,
                "message": message,
                "span": span,
            }
        )

    def lint_routine(
        self,
        routine: dict[str, Any],
        span: dict[str, Any] | None,
    ) -> None:
        name = routine.get("name", "")
        taking_ms = routine.get("taking_ms")

        # 1. Naming convention enforcement
        if self.strict_naming and "telemetry" in self.file_path.lower():
            allowed_prefixes = ("telemetry_", "log_", "init_", "emit_")
            if not any(name.startswith(p) for p in allowed_prefixes):
                self.add_diagnostic(
                    "Warning",
                    f"Python IPC Plugin: Routine '{name}' in telemetry domain must follow standard prefixes: {', '.join(allowed_prefixes)}",
                    span,
                )

        # 2. WCET contract validation
        if self.require_wcet:
            if taking_ms is None:
                self.add_diagnostic(
                    "Warning",
                    f"Python IPC Plugin: Missing WCET annotation ('taking <N>ms') on routine '{name}'. Static scheduler cannot verify deadlines.",
                    span,
                )
            elif taking_ms == 0:
                self.add_diagnostic(
                    "Warning",
                    f"Python IPC Plugin: Zero execution budget ('taking 0ms') on routine '{name}' is invalid for non-instantaneous routines.",
                    span,
                )

        # Recursively audit routine body
        for stmt in routine.get("body", []):
            self.lint_spanned_statement(stmt)

    def lint_isolate(
        self,
        isolate: dict[str, Any],
        span: dict[str, Any] | None,
    ) -> None:
        name = isolate.get("name") or "anonymous"
        manifest = isolate.get("manifest", {})
        cpu_budget = manifest.get("cpu_budget_ms")

        # Check for unconstrained isolate CPU allocations
        if cpu_budget is None:
            self.add_diagnostic(
                "Note",
                f"Python IPC Plugin: Sandbox isolate '{name}' does not specify an explicit 'enable cpu(...)' resource ceiling.",
                span,
            )

        for stmt in isolate.get("body", []):
            self.lint_spanned_statement(stmt)

    def lint_match_entropy(
        self,
        match_stmt: dict[str, Any],
        span: dict[str, Any] | None,
    ) -> None:
        # Verify all entropic states are safely reconciled
        if not match_stmt.get("decayed_branch"):
            self.add_diagnostic(
                "Warning",
                "Python IPC Plugin: 'match entropy' construct lacks a 'Decayed' branch handler, risking unhandled entropic drift.",
                span,
            )

    def lint_spanned_statement(self, spanned: dict[str, Any]) -> None:
        if not isinstance(spanned, dict):
            return

        stmt = spanned.get("stmt", {})
        span = spanned.get("span")

        if "RoutineDef" in stmt:
            self.lint_routine(stmt["RoutineDef"], span)
        elif "Isolate" in stmt:
            self.lint_isolate(stmt["Isolate"], span)
        elif "MatchEntropy" in stmt:
            self.lint_match_entropy(stmt["MatchEntropy"], span)
        elif "Using" in stmt:
            for child in stmt["Using"].get("body", []):
                self.lint_spanned_statement(child)
        elif "RelativisticBlock" in stmt:
            for child in stmt["RelativisticBlock"].get("body", []):
                self.lint_spanned_statement(child)

    def run(self) -> dict[str, Any]:
        for tb in self.ast.get("timelines", []):
            for spanned in tb.get("statements", []):
                self.lint_spanned_statement(spanned)

        return {
            "protocol_version": self.request.get("protocol_version", "0.1.0"),
            "compiler_version": self.request.get("compiler_version", "0.1.0-alpha.1"),
            "status": "Success",
            "diagnostics": self.diagnostics,
            "modified_ast": self.ast,
        }


def main() -> None:
    try:
        raw_input = sys.stdin.read()
        if not raw_input.strip():
            sys.exit(0)
        request = json.loads(raw_input)
        linter = CausmAstLinter(request)
        response = linter.run()
        sys.stdout.write(json.dumps(response))
        sys.stdout.flush()
    except (json.JSONDecodeError, KeyError, TypeError) as exc:
        err_response = {
            "protocol_version": "0.1.0",
            "compiler_version": "0.1.0-alpha.1",
            "status": {"Error": f"Linter IPC error: {exc}"},
            "diagnostics": [],
            "modified_ast": None,
        }
        sys.stdout.write(json.dumps(err_response))
        sys.stdout.flush()


if __name__ == "__main__":
    main()
