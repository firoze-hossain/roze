"""Instruction disassembly, static bytecode classification, and suspicious pattern identification."""

import struct
from dataclasses import dataclass
from typing import List, Dict, Optional, Tuple
from .logger import logger

try:
    import capstone
    HAS_CAPSTONE = True
except ImportError:
    HAS_CAPSTONE = False

@dataclass
class DisassembledInstruction:
    address: int
    mnemonic: str
    op_str: str
    size: int
    raw_bytes: bytes
    category: str  # "CONTROL_FLOW", "STACK", "SYSTEM", "ARITHMETIC", "DATA", "MISC"

class InstructionClassifier:
    """Classifies x86/x64 instructions into defensive behavioral categories."""

    CONTROL_FLOW_MNEMONICS = {
        "jmp", "ljmp", "call", "ret", "retn", "retf", "syscall", "sysenter", "int",
        "je", "jne", "jz", "jnz", "jg", "jge", "jl", "jle", "ja", "jae", "jb", "jbe",
        "js", "jns", "jo", "jno", "loop", "loope", "loopne",
    }
    
    STACK_MNEMONICS = {
        "push", "pop", "pushfq", "popfq", "enter", "leave",
    }
    
    SYSTEM_MNEMONICS = {
        "syscall", "sysenter", "int3", "into", "cpuid", "rdtsc", "rdtscp",
        "in", "out", "cli", "sti", "invlpg", "wrmsr", "rdmsr",
    }
    
    SUSPICIOUS_API_PATTERNS = [
        b"VirtualAlloc", b"VirtualProtect", b"WriteProcessMemory",
        b"CreateRemoteThread", b"NtQueueApcThread", b"RtlMoveMemory",
        b"mprotect", b"mmap", b"ptrace", b"process_vm_writev",
    ]

    @classmethod
    def categorize(cls, mnemonic: str) -> str:
        m = mnemonic.lower()
        if m in cls.CONTROL_FLOW_MNEMONICS:
            return "CONTROL_FLOW"
        if m in cls.STACK_MNEMONICS:
            return "STACK"
        if m in cls.SYSTEM_MNEMONICS:
            return "SYSTEM"
        if m in ("mov", "lea", "xchg", "cmov"):
            return "DATA"
        if m in ("add", "sub", "xor", "and", "or", "shl", "shr", "imul", "idiv", "inc", "dec", "not", "neg"):
            return "ARITHMETIC"
        return "MISC"

def disassemble_buffer(code_bytes: bytes, base_address: int = 0x400000, max_instructions: int = 100) -> List[DisassembledInstruction]:
    """Disassembles machine bytecode using Capstone or lightweight fallback."""
    if not code_bytes:
        return []
        
    if HAS_CAPSTONE:
        return _disassemble_capstone(code_bytes, base_address, max_instructions)
    else:
        return _disassemble_pure_python(code_bytes, base_address, max_instructions)

def _disassemble_capstone(code_bytes: bytes, base_address: int, max_instructions: int) -> List[DisassembledInstruction]:
    instructions = []
    try:
        md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
        count = 0
        for insn in md.disasm(code_bytes, base_address):
            category = InstructionClassifier.categorize(insn.mnemonic)
            instructions.append(
                DisassembledInstruction(
                    address=insn.address,
                    mnemonic=insn.mnemonic,
                    op_str=insn.op_str,
                    size=insn.size,
                    raw_bytes=bytes(insn.bytes),
                    category=category,
                )
            )
            count += 1
            if count >= max_instructions:
                break
    except Exception as e:
        logger.warning(f"Capstone disassembly encountered error: {e}")
    return instructions

def _disassemble_pure_python(code_bytes: bytes, base_address: int, max_instructions: int) -> List[DisassembledInstruction]:
    """Lightweight pure-Python x86_64 instruction decoder fallback."""
    instructions = []
    offset = 0
    total_len = len(code_bytes)
    
    # Common opcodes mapping
    OPCODES = {
        0x90: ("nop", "", 1),
        0xCC: ("int3", "", 1),
        0xC3: ("ret", "", 1),
        0xCB: ("retf", "", 1),
        0xC9: ("leave", "", 1),
        0x0F: None,  # Multi-byte prefix
    }
    
    count = 0
    while offset < total_len and count < max_instructions:
        addr = base_address + offset
        b = code_bytes[offset]
        
        # Check single-byte return or interrupt
        if b in OPCODES and OPCODES[b] is not None:
            mnem, op_s, sz = OPCODES[b]
            instructions.append(
                DisassembledInstruction(
                    address=addr,
                    mnemonic=mnem,
                    op_str=op_s,
                    size=sz,
                    raw_bytes=code_bytes[offset : offset + sz],
                    category=InstructionClassifier.categorize(mnem),
                )
            )
            offset += sz
            count += 1
            continue
            
        # PUSH reg (0x50 - 0x57)
        if 0x50 <= b <= 0x57:
            reg_names = ["rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi"]
            reg = reg_names[b - 0x50]
            instructions.append(
                DisassembledInstruction(
                    address=addr,
                    mnemonic="push",
                    op_str=reg,
                    size=1,
                    raw_bytes=code_bytes[offset : offset + 1],
                    category="STACK",
                )
            )
            offset += 1
            count += 1
            continue
            
        # POP reg (0x58 - 0x5F)
        if 0x58 <= b <= 0x5F:
            reg_names = ["rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi"]
            reg = reg_names[b - 0x58]
            instructions.append(
                DisassembledInstruction(
                    address=addr,
                    mnemonic="pop",
                    op_str=reg,
                    size=1,
                    raw_bytes=code_bytes[offset : offset + 1],
                    category="STACK",
                )
            )
            offset += 1
            count += 1
            continue
            
        # SYSCALL (0x0F 0x05)
        if b == 0x0F and offset + 1 < total_len and code_bytes[offset + 1] == 0x05:
            instructions.append(
                DisassembledInstruction(
                    address=addr,
                    mnemonic="syscall",
                    op_str="",
                    size=2,
                    raw_bytes=code_bytes[offset : offset + 2],
                    category="SYSTEM",
                )
            )
            offset += 2
            count += 1
            continue
            
        # Advance 1 byte default
        instructions.append(
            DisassembledInstruction(
                address=addr,
                mnemonic="db",
                op_str=f"0x{b:02x}",
                size=1,
                raw_bytes=code_bytes[offset : offset + 1],
                category="DATA",
            )
        )
        offset += 1
        count += 1
        
    return instructions

def scan_suspicious_api_strings(data: bytes) -> List[Tuple[int, str]]:
    """Identifies occurrences of suspicious systems and memory management APIs."""
    found = []
    for api in InstructionClassifier.SUSPICIOUS_API_PATTERNS:
        start = 0
        while True:
            idx = data.find(api, start)
            if idx == -1:
                break
            found.append((idx, api.decode("ascii", errors="replace")))
            start = idx + len(api)
    return found
