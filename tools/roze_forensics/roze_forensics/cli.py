"""Command-line interface for Roze Memory Forensics & Defensive Analysis Harness."""

import sys
import os
import argparse
from typing import Optional
from .config import ForensicsConfig
from .logger import setup_logger, logger
from .memory_reader import ProcessMemoryReader, MemoryRegion
from .dump_parser import MemoryDumpParser
from .auditor import MemoryAuditor
from .control_flow import ControlFlowAuditor
from .disasm import disassemble_buffer
from .patterns import compile_hex_pattern, find_hex_pattern, YaraScanner
from .emulator import SandboxedCpuEmulator
from .reporter import ForensicsAuditReport

def main(args: Optional[list] = None):
    parser = argparse.ArgumentParser(
        prog="roze-forensics",
        description="Roze M3-T06: Production-Grade Defensive Memory Forensics & Analysis Harness",
    )
    
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--pid", type=int, help="Target process ID for live read-only inspection")
    group.add_argument("--dump", type=str, help="Path to offline memory dump or core file")

    parser.add_argument("--pattern", type=str, help="Search for hex pattern with wildcards (e.g. '48 89 ?? ?? c3')")
    parser.add_argument("--yara", type=str, help="Path to YARA rule file for signature scanning")
    parser.add_argument("--entropy-threshold", type=float, default=7.2, help="Shannon entropy alert threshold (default: 7.2)")
    parser.add_argument("--json", action="store_true", help="Output results formatted as JSON")
    parser.add_argument("--output", type=str, help="Path to write report output")
    parser.add_argument("--audit-log", type=str, help="Path to append structured audit log")

    parsed = parser.parse_args(args)
    
    if parsed.audit_log:
        setup_logger(log_file=parsed.audit_log)

    config = ForensicsConfig(entropy_threshold=parsed.entropy_threshold)
    auditor = MemoryAuditor(config=config)
    cf_auditor = ControlFlowAuditor()

    regions = []
    reader = None
    target_id = ""
    target_type = ""

    if parsed.pid:
        target_id = f"PID {parsed.pid}"
        target_type = "PROCESS"
        reader = ProcessMemoryReader(parsed.pid)
        regions = reader.enumerate_regions()
    elif parsed.dump:
        target_id = parsed.dump
        target_type = "MEMORY_DUMP"
        try:
            reader = MemoryDumpParser(parsed.dump)
            regions = reader.get_memory_regions()
        except Exception as e:
            logger.error(f"Failed to load memory dump {parsed.dump}: {e}")
            sys.exit(1)

    total_bytes = sum(r.size for r in regions)
    exec_count = sum(1 for r in regions if r.is_executable)
    rwx_count = sum(1 for r in regions if r.is_rwx)

    anomalies = auditor.audit_all_regions(regions, reader=reader)

    # Disassembly & Control-Flow Analysis on Anomalous Executable Regions
    cf_anomalies = []
    pattern_matches = []

    compiled_pattern = None
    if parsed.pattern:
        try:
            compiled_pattern = compile_hex_pattern(parsed.pattern)
        except Exception as e:
            logger.error(f"Invalid hex pattern: {e}")

    yara_scanner = None
    if parsed.yara:
        yara_scanner = YaraScanner(parsed.yara)

    for r in regions:
        if (r.is_executable or r.is_rwx) and r.is_readable and reader:
            try:
                sample_len = min(r.size, 8192)
                code_sample = reader.read_bytes(r.start_address, sample_len)
                if code_sample:
                    # Control flow analysis
                    insns = disassemble_buffer(code_sample, base_address=r.start_address, max_instructions=50)
                    cf_anomalies.extend(cf_auditor.audit_instructions(insns))
                    
                    # Pattern scanning
                    if compiled_pattern:
                        m_list = find_hex_pattern(code_sample, compiled_pattern)
                        for m in m_list:
                            pattern_matches.append({
                                "address": f"0x{r.start_address + m.offset:x}",
                                "pattern": parsed.pattern,
                                "matched_hex": m.matched_bytes.hex(),
                            })
                            
                    # YARA scanning
                    if yara_scanner and yara_scanner.enabled:
                        y_matches = yara_scanner.scan_bytes(code_sample)
                        for ym in y_matches:
                            pattern_matches.append({
                                "address": f"0x{r.start_address:x}",
                                "yara_rule": ym["rule"],
                                "tags": ym["tags"],
                            })
            except Exception as e:
                logger.debug(f"Error inspecting region at 0x{r.start_address:x}: {e}")

    if reader:
        reader.close()

    report = ForensicsAuditReport(
        target_identifier=target_id,
        target_type=target_type,
        total_regions_enumerated=len(regions),
        total_memory_bytes=total_bytes,
        executable_regions_count=exec_count,
        writable_executable_count=rwx_count,
        anomalies=anomalies,
        control_flow_anomalies=cf_anomalies,
        pattern_matches=pattern_matches,
    )

    output_text = report.to_json() if parsed.json else report.format_ascii_table()

    if parsed.output:
        try:
            with open(parsed.output, "w", encoding="utf-8") as out_f:
                out_f.write(output_text + "\n")
            print(f"Report written to {parsed.output}")
        except Exception as e:
            logger.error(f"Failed to write report to {parsed.output}: {e}")
    else:
        print(output_text)

if __name__ == "__main__":
    main()
