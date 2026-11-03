"""Memory permission auditing, anomaly detection, and suspicious region classification."""

import os
from dataclasses import dataclass, field
from typing import List, Dict, Optional
from .memory_reader import MemoryRegion
from .entropy import calculate_shannon_entropy
from .binary_parser import detect_in_memory_binary
from .config import ForensicsConfig, DEFAULT_CONFIG
from .logger import logger

@dataclass
class MemoryAnomaly:
    anomaly_type: str
    severity: str  # "CRITICAL", "HIGH", "MEDIUM", "LOW"
    start_address: int
    size: int
    description: str
    region_permissions: str
    pathname: str = ""
    metadata: Dict = field(default_factory=dict)

    def to_dict(self) -> dict:
        return {
            "anomaly_type": self.anomaly_type,
            "severity": self.severity,
            "start_address": f"0x{self.start_address:016x}",
            "size": self.size,
            "permissions": self.region_permissions,
            "pathname": self.pathname,
            "description": self.description,
            "metadata": self.metadata,
        }

class MemoryAuditor:
    """Audits process memory maps and segments for defensive indicators of compromise."""

    def __init__(self, config: ForensicsConfig = DEFAULT_CONFIG):
        self.config = config

    def audit_region(self, region: MemoryRegion, sample_bytes: Optional[bytes] = None) -> List[MemoryAnomaly]:
        """Inspects a single memory region for defensive anomalies."""
        anomalies = []
        
        # 1. W+X / RWX Permission Violation
        if self.config.detect_rwx_violations and region.is_rwx:
            anomalies.append(
                MemoryAnomaly(
                    anomaly_type="RWX_PERMISSION_VIOLATION",
                    severity="CRITICAL",
                    start_address=region.start_address,
                    size=region.size,
                    description="Memory region is simultaneously writable and executable (W+X violation).",
                    region_permissions=region.permissions,
                    pathname=region.pathname,
                )
            )

        # 2. Executable Stack Detection
        if region.is_executable and "[stack" in region.pathname:
            anomalies.append(
                MemoryAnomaly(
                    anomaly_type="EXECUTABLE_STACK_DETECTED",
                    severity="CRITICAL",
                    start_address=region.start_address,
                    size=region.size,
                    description="The call stack region has executable permissions enabled.",
                    region_permissions=region.permissions,
                    pathname=region.pathname,
                )
            )

        # 3. Executable Heap Detection
        if region.is_executable and "[heap]" in region.pathname:
            anomalies.append(
                MemoryAnomaly(
                    anomaly_type="EXECUTABLE_HEAP_DETECTED",
                    severity="HIGH",
                    start_address=region.start_address,
                    size=region.size,
                    description="The process heap region has executable permissions enabled.",
                    region_permissions=region.permissions,
                    pathname=region.pathname,
                )
            )

        # 4. Unbacked Executable Memory (Anonymous Code)
        if self.config.detect_unbacked_exec and region.is_executable and region.is_anonymous:
            # Check if not stack/heap
            if not region.pathname.startswith("["):
                anomalies.append(
                    MemoryAnomaly(
                        anomaly_type="UNBACKED_EXECUTABLE_MEMORY",
                        severity="HIGH",
                        start_address=region.start_address,
                        size=region.size,
                        description="Executable memory region is not backed by any mapped file on disk.",
                        region_permissions=region.permissions,
                        pathname="[anonymous]",
                    )
                )

        # 5. Content-Level Inspection (if sample bytes available)
        if sample_bytes and len(sample_bytes) > 0:
            # Check for High Entropy Executable Region (packed / encrypted code)
            if self.config.detect_high_entropy_exec and region.is_executable:
                entropy = calculate_shannon_entropy(sample_bytes)
                if entropy >= self.config.entropy_threshold:
                    anomalies.append(
                        MemoryAnomaly(
                            anomaly_type="HIGH_ENTROPY_EXECUTABLE_PAGE",
                            severity="HIGH",
                            start_address=region.start_address,
                            size=len(sample_bytes),
                            description=f"Executable memory block exhibits high Shannon entropy ({entropy:.2f}/8.0), indicating packed or encrypted payload.",
                            region_permissions=region.permissions,
                            pathname=region.pathname,
                            metadata={"entropy": round(entropy, 2)},
                        )
                    )

            # Check for In-Memory Injected Binary Headers in unbacked or writable space
            bin_type = detect_in_memory_binary(sample_bytes)
            if bin_type in ("ELF", "PE") and (region.is_anonymous or region.is_writable):
                anomalies.append(
                    MemoryAnomaly(
                        anomaly_type=f"UNMAPPED_{bin_type}_HEADER",
                        severity="CRITICAL",
                        start_address=region.start_address,
                        size=region.size,
                        description=f"Standalone {bin_type} executable image detected within unbacked or writable memory.",
                        region_permissions=region.permissions,
                        pathname=region.pathname,
                        metadata={"binary_format": bin_type},
                    )
                )

        return anomalies

    def audit_all_regions(self, regions: List[MemoryRegion], reader=None) -> List[MemoryAnomaly]:
        """Audits a complete list of memory regions, pulling sample bytes when possible."""
        all_anomalies = []
        for region in regions:
            sample_bytes = None
            if reader and (region.is_executable or region.is_rwx) and region.is_readable:
                try:
                    read_len = min(region.size, 4096)
                    sample_bytes = reader.read_bytes(region.start_address, read_len)
                except Exception:
                    sample_bytes = None
            all_anomalies.extend(self.audit_region(region, sample_bytes))
        return all_anomalies
