"""Roze Memory Forensics & Defensive Analysis Harness (M3-T06)."""

from .config import ForensicsConfig, DEFAULT_CONFIG
from .logger import setup_logger, logger
from .memory_reader import ProcessMemoryReader, MemoryRegion
from .dump_parser import MemoryDumpParser, DumpSegment
from .entropy import calculate_shannon_entropy, analyze_entropy_blocks, is_high_entropy
from .patterns import (
    compile_hex_pattern,
    find_hex_pattern,
    search_strings,
    search_wide_strings,
    YaraScanner,
)
from .binary_parser import detect_in_memory_binary, parse_in_memory_elf, parse_in_memory_pe
from .auditor import MemoryAuditor, MemoryAnomaly
from .disasm import disassemble_buffer, InstructionClassifier
from .control_flow import ControlFlowAuditor, ControlFlowAnomaly
from .emulator import SandboxedCpuEmulator, CpuState
from .reporter import ForensicsAuditReport

__all__ = [
    "ForensicsConfig",
    "DEFAULT_CONFIG",
    "setup_logger",
    "logger",
    "ProcessMemoryReader",
    "MemoryRegion",
    "MemoryDumpParser",
    "DumpSegment",
    "calculate_shannon_entropy",
    "analyze_entropy_blocks",
    "is_high_entropy",
    "compile_hex_pattern",
    "find_hex_pattern",
    "search_strings",
    "search_wide_strings",
    "YaraScanner",
    "detect_in_memory_binary",
    "parse_in_memory_elf",
    "parse_in_memory_pe",
    "MemoryAuditor",
    "MemoryAnomaly",
    "disassemble_buffer",
    "InstructionClassifier",
    "ControlFlowAuditor",
    "ControlFlowAnomaly",
    "SandboxedCpuEmulator",
    "CpuState",
    "ForensicsAuditReport",
]
