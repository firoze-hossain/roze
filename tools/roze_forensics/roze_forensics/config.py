"""Configuration settings and defaults for Roze Memory Forensics Toolkit."""

from dataclasses import dataclass, field
from typing import List, Optional

@dataclass
class ForensicsConfig:
    # Memory acquisition settings
    max_region_size_bytes: int = 100 * 1024 * 1024  # 100 MB max per region
    scan_anonymous_regions: bool = True
    scan_heap: bool = True
    scan_stack: bool = True
    
    # Anomaly detection thresholds
    entropy_threshold: float = 7.2  # Shannon entropy threshold (0.0 to 8.0)
    entropy_block_size: int = 4096  # Block size for entropy analysis
    detect_rwx_violations: bool = True
    detect_unbacked_exec: bool = True
    detect_high_entropy_exec: bool = True
    
    # Defensive control-flow & bytecode analysis
    disassemble_anomalous_regions: bool = True
    max_disassembly_instructions: int = 100
    detect_control_flow_anomalies: bool = True
    indirect_branch_density_threshold: float = 0.4  # Ratio of branch/ret instructions
    
    # Sandboxed emulation harness
    max_emulation_steps: int = 2000
    emulation_memory_limit_bytes: int = 16 * 1024 * 1024  # 16 MB sandbox memory
    strict_bounds_checking: bool = True
    
    # Reporting and audit logging
    log_level: str = "INFO"
    audit_log_path: Optional[str] = None
    report_json: bool = True
    report_table: bool = True
    yara_rules_path: Optional[str] = None

DEFAULT_CONFIG = ForensicsConfig()
