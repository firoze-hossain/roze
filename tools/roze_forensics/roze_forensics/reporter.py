"""Structured JSON reporting and formatted ASCII summary tables for forensics investigations."""

import json
from dataclasses import dataclass, field
from typing import List, Dict, Any, Optional
from .memory_reader import MemoryRegion
from .auditor import MemoryAnomaly
from .control_flow import ControlFlowAnomaly

@dataclass
class ForensicsAuditReport:
    target_identifier: str
    target_type: str  # "PROCESS" or "MEMORY_DUMP"
    total_regions_enumerated: int
    total_memory_bytes: int
    executable_regions_count: int
    writable_executable_count: int
    anomalies: List[MemoryAnomaly] = field(default_factory=list)
    control_flow_anomalies: List[ControlFlowAnomaly] = field(default_factory=list)
    pattern_matches: List[Dict[str, Any]] = field(default_factory=list)
    metadata: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "target_identifier": self.target_identifier,
            "target_type": self.target_type,
            "statistics": {
                "total_regions": self.total_regions_enumerated,
                "total_memory_bytes": self.total_memory_bytes,
                "executable_regions": self.executable_regions_count,
                "rwx_violations": self.writable_executable_count,
                "total_anomalies": len(self.anomalies),
                "control_flow_anomalies": len(self.control_flow_anomalies),
                "pattern_matches": len(self.pattern_matches),
            },
            "anomalies": [a.to_dict() for a in self.anomalies],
            "control_flow_anomalies": [
                {
                    "anomaly_type": cfa.anomaly_type,
                    "severity": cfa.severity,
                    "address": f"0x{cfa.address:016x}",
                    "sequence": cfa.instruction_sequence,
                    "description": cfa.description,
                }
                for cfa in self.control_flow_anomalies
            ],
            "pattern_matches": self.pattern_matches,
            "metadata": self.metadata,
        }

    def to_json(self, indent: int = 2) -> str:
        return json.dumps(self.to_dict(), indent=indent)

    def format_ascii_table(self) -> str:
        """Renders an aligned ASCII audit report for terminal view."""
        lines = []
        border = "=" * 80
        lines.append(border)
        lines.append(f" ROZE MEMORY FORENSICS & DEFENSIVE AUDIT REPORT: {self.target_identifier}")
        lines.append(border)
        lines.append(f" Target Type           : {self.target_type}")
        lines.append(f" Total Mapped Regions  : {self.total_regions_enumerated}")
        lines.append(f" Total Address Space   : {self.total_memory_bytes / (1024*1024):.2f} MB")
        lines.append(f" Executable Regions    : {self.executable_regions_count}")
        lines.append(f" RWX (W+X) Violations  : {self.writable_executable_count}")
        lines.append(f" Memory Anomalies      : {len(self.anomalies)}")
        lines.append(f" Control Flow Anomalies: {len(self.control_flow_anomalies)}")
        lines.append("-" * 80)
        
        if not self.anomalies:
            lines.append(" [✓] No critical memory anomalies detected.")
        else:
            lines.append(" DETECTED DEFENSIVE MEMORY ANOMALIES:")
            lines.append(f" {'SEVERITY':<10} {'START ADDRESS':<18} {'PERMS':<7} {'ANOMALY TYPE'}")
            lines.append(f" {'-'*8:<10} {'-'*16:<18} {'-'*5:<7} {'-'*30}")
            for a in self.anomalies:
                lines.append(
                    f" {a.severity:<10} 0x{a.start_address:016x}   {a.region_permissions:<7} {a.anomaly_type}"
                )
                lines.append(f"   └─ Details: {a.description}")

        if self.control_flow_anomalies:
            lines.append("-" * 80)
            lines.append(" DEFENSIVE CONTROL-FLOW ANOMALIES:")
            for cfa in self.control_flow_anomalies:
                lines.append(
                    f" [{cfa.severity}] 0x{cfa.address:016x} | {cfa.anomaly_type}: {cfa.instruction_sequence}"
                )
                lines.append(f"   └─ {cfa.description}")

        lines.append(border)
        return "\n".join(lines)
