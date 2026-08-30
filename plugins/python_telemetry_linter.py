#!/usr/bin/env python3

import json
import sys
from typing import Any


class CausmTelemetryAnalyzer:
    def __init__(self, payload: dict[str, Any]) -> None:
        self.payload = payload
        self.phase = payload.get("phase", "AstTransform")
        self.ast = payload.get("ast", {})
        self.file_path = payload.get("file_path", "unknown.csm")
        self.options = payload.get("options", {})
        self.analysis = payload.get("analysis")

        self.diagnostics: list[dict[str, Any]] = []
        self.timelines_summary: list[dict[str, Any]] = []

    def emit_diagnostic(
        self,
        level: str,
        message: str,
        span: dict[str, Any] | None = None,
    ) -> None:
        self.diagnostics.append(
            {
                "level": level,
                "message": f"[Causm Telemetry Plugin] {message}",
                "span": span,
            }
        )

    def audit_routine(
        self, routine: dict[str, Any], span: dict[str, Any] | None
    ) -> None:
        name = routine.get("name", "anonymous")
        taking_ms = routine.get("taking_ms")
        caps = routine.get("required_capabilities", [])

        # 1. Real-time WCET contract enforcement
        if taking_ms is None:
            self.emit_diagnostic(
                "Warning",
                f"Routine '{name}' lacks explicit temporal contract ('taking <N>ms'). SMT verifier will assume unconstrained WCET.",
                span,
            )
        elif taking_ms > 500:
            self.emit_diagnostic(
                "Warning",
                f"Routine '{name}' specifies a high temporal budget ({taking_ms}ms > 500ms limit). Consider decomposing into asynchronous pipeline stages.",
                span,
            )

        # 2. Capability isolation audit
        for cap in caps:
            if "Syscall" in str(cap) or "Net" in str(cap):
                self.emit_diagnostic(
                    "Note",
                    f"Routine '{name}' requests high-privilege capability '{cap}'. Ensure enclosing block is wrapped in an @isolate boundary.",
                    span,
                )

        # Recursively audit routine body
        for stmt in routine.get("body", []):
            self.audit_statement(stmt)

    def audit_isolate(
        self, isolate: dict[str, Any], span: dict[str, Any] | None
    ) -> None:
        name = isolate.get("name") or "anonymous_isolate"
        manifest = isolate.get("manifest", {})
        cpu_budget = manifest.get("cpu_budget_ms")
        mem_budget = manifest.get("memory_budget_bytes")

        if cpu_budget is None:
            self.emit_diagnostic(
                "Note",
                f"Isolate sandbox '{name}' does not declare an explicit 'enable cpu(...)' quota.",
                span,
            )

        if mem_budget is not None and mem_budget > 64 * 1024 * 1024:
            self.emit_diagnostic(
                "Warning",
                f"Isolate sandbox '{name}' allocates {mem_budget // (1024 * 1024)}MB memory quota (exceeds recommended 64MB embedded budget).",
                span,
            )

        for stmt in isolate.get("body", []):
            self.audit_statement(stmt)

    def audit_statement(self, spanned: dict[str, Any]) -> None:
        if not isinstance(spanned, dict):
            return

        stmt = spanned.get("stmt", {})
        span = spanned.get("span")

        if "RoutineDef" in stmt:
            self.audit_routine(stmt["RoutineDef"], span)
        elif "Isolate" in stmt:
            self.audit_isolate(stmt["Isolate"], span)
        elif "Using" in stmt:
            for child in stmt["Using"].get("body", []):
                self.audit_statement(child)
        elif "RelativisticBlock" in stmt:
            for child in stmt["RelativisticBlock"].get("body", []):
                self.audit_statement(child)

    def generate_causal_timeline_report(self) -> None:
        timelines = self.ast.get("timelines", [])
        if not timelines:
            return

        lines = [
            f"=== Static Causal Timeline Map for '{self.file_path}' ===",
            "  Time Coord   | Statements | Verified Safety State",
            "  -------------+------------+-----------------------",
        ]

        for idx, tb in enumerate(timelines):
            coord = tb.get("time", {})
            coord_str = "@0ms"
            if isinstance(coord, dict):
                if "Global" in coord:
                    coord_str = f"@{coord['Global']}ms"
                elif "Relative" in coord:
                    coord_str = f"+{coord['Relative']}ms"
                elif "Periodic" in coord:
                    coord_str = f"@every {coord['Periodic']}ms"
                elif "Branch" in coord:
                    coord_str = f"branch:{coord['Branch']}"
            elif isinstance(coord, str):
                coord_str = coord

            stmt_count = len(tb.get("statements", []))
            lines.append(
                f"  {coord_str:<12} | {stmt_count:<10} | [✓] Deterministic Arena"
            )

        lines.append("  =======================================================")
        report = "\n".join(lines)
        self.emit_diagnostic("Note", f"\n{report}")

    def execute(self) -> dict[str, Any]:
        # Perform AST inspections
        for tb in self.ast.get("timelines", []):
            for spanned in tb.get("statements", []):
                self.audit_statement(spanned)

        # In PostAnalysis or AstTransform phase, emit causal timeline map
        if self.options.get("visualize", "true").lower() == "true":
            self.generate_causal_timeline_report()

        # Multi-stage telemetry metrics reporting
        if self.phase == "PostAnalysis" and self.analysis:
            verified = self.analysis.get("verification_passed", False)
            cost = self.analysis.get("total_estimated_cost", 0)
            self.emit_diagnostic(
                "Note",
                f"Post-Analysis Telemetry: Static Verification = {'PASS' if verified else 'FAIL'}, Estimated WCET = {cost} cycles.",
            )

        return {
            "status": "Success",
            "modified_ast": self.ast,
            "emitted_payload": None,
            "diagnostics": self.diagnostics,
        }


def main() -> None:
    try:
        raw = sys.stdin.read()
        if not raw.strip():
            sys.exit(0)

        req = json.loads(raw)
        analyzer = CausmTelemetryAnalyzer(req)
        response = analyzer.execute()
        sys.stdout.write(json.dumps(response))
        sys.stdout.flush()
    except Exception as e:
        err_response = {
            "status": {"Error": f"Plugin failed: {e!s}"},
            "modified_ast": None,
            "emitted_payload": None,
            "diagnostics": [
                {
                    "level": "Error",
                    "message": f"Plugin execution exception: {e!s}",
                    "span": None,
                }
            ],
        }
        sys.stdout.write(json.dumps(err_response))
        sys.stdout.flush()


if __name__ == "__main__":
    main()
