"""Restricted CPU emulation environment for safe behavioral analysis without host execution."""

from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple
from .logger import logger

@dataclass
class CpuState:
    rax: int = 0
    rbx: int = 0
    rcx: int = 0
    rdx: int = 0
    rsi: int = 0
    rdi: int = 0
    rbp: int = 0
    rsp: int = 0
    r8: int = 0
    r9: int = 0
    r10: int = 0
    r11: int = 0
    r12: int = 0
    r13: int = 0
    r14: int = 0
    r15: int = 0
    rip: int = 0
    rflags: int = 0x202  # IF flag set
    
    def to_dict(self) -> Dict[str, str]:
        return {
            "rax": f"0x{self.rax:016x}",
            "rbx": f"0x{self.rbx:016x}",
            "rcx": f"0x{self.rcx:016x}",
            "rdx": f"0x{self.rdx:016x}",
            "rsi": f"0x{self.rsi:016x}",
            "rdi": f"0x{self.rdi:016x}",
            "rbp": f"0x{self.rbp:016x}",
            "rsp": f"0x{self.rsp:016x}",
            "rip": f"0x{self.rip:016x}",
            "rflags": f"0x{self.rflags:08x}",
        }

class SandboxedCpuEmulator:
    """Isolated, pure-software CPU state tracker and instruction emulator."""

    def __init__(self, memory_size: int = 64 * 1024, base_address: int = 0x400000, max_steps: int = 1000):
        self.memory_size = memory_size
        self.base_address = base_address
        self.max_steps = max_steps
        self.memory = bytearray(memory_size)
        self.cpu = CpuState()
        self.cpu.rip = base_address
        # Initialize stack at the high end of sandbox memory
        self.cpu.rsp = base_address + memory_size - 128
        self.cpu.rbp = self.cpu.rsp
        self.steps_executed = 0
        self.halted = False
        self.halt_reason = ""
        self.execution_trace: List[str] = []

    def load_bytes(self, data: bytes, target_address: int = 0x400000):
        """Loads machine bytecode into sandbox memory at specified address."""
        if target_address < self.base_address or target_address + len(data) > self.base_address + self.memory_size:
            raise ValueError("Data exceeds sandbox memory boundaries.")
        offset = target_address - self.base_address
        self.memory[offset : offset + len(data)] = data

    def _read_mem(self, addr: int, size: int) -> int:
        if addr < self.base_address or addr + size > self.base_address + self.memory_size:
            self.halted = True
            self.halt_reason = f"Memory read access violation at 0x{addr:x}"
            return 0
        offset = addr - self.base_address
        val = 0
        for i in range(size):
            val |= (self.memory[offset + i] << (i * 8))
        return val

    def _write_mem(self, addr: int, val: int, size: int):
        if addr < self.base_address or addr + size > self.base_address + self.memory_size:
            self.halted = True
            self.halt_reason = f"Memory write access violation at 0x{addr:x}"
            return
        offset = addr - self.base_address
        for i in range(size):
            self.memory[offset + i] = (val >> (i * 8)) & 0xFF

    def push(self, val: int):
        self.cpu.rsp -= 8
        self._write_mem(self.cpu.rsp, val, 8)

    def pop(self) -> int:
        val = self._read_mem(self.cpu.rsp, 8)
        self.cpu.rsp += 8
        return val

    def step(self) -> bool:
        """Executes a single instruction within safe software bounds."""
        if self.halted or self.steps_executed >= self.max_steps:
            if not self.halted:
                self.halted = True
                self.halt_reason = f"Step limit ({self.max_steps}) exceeded"
            return False

        if self.cpu.rip < self.base_address or self.cpu.rip >= self.base_address + self.memory_size:
            self.halted = True
            self.halt_reason = f"Instruction pointer out of bounds at 0x{self.cpu.rip:x}"
            return False

        pc_offset = self.cpu.rip - self.base_address
        b0 = self.memory[pc_offset]

        # NOP (0x90)
        if b0 == 0x90:
            self.execution_trace.append(f"0x{self.cpu.rip:x}: nop")
            self.cpu.rip += 1
            self.steps_executed += 1
            return True

        # INT3 (0xCC) or HLT (0xF4)
        if b0 in (0xCC, 0xF4):
            mnemonic = "int3" if b0 == 0xCC else "hlt"
            self.execution_trace.append(f"0x{self.cpu.rip:x}: {mnemonic}")
            self.cpu.rip += 1
            self.halted = True
            self.halt_reason = f"Halted by {mnemonic}"
            self.steps_executed += 1
            return False

        # RET (0xC3)
        if b0 == 0xC3:
            ret_addr = self.pop()
            self.execution_trace.append(f"0x{self.cpu.rip:x}: ret -> 0x{ret_addr:x}")
            self.cpu.rip = ret_addr
            self.steps_executed += 1
            return True

        # PUSH reg (0x50 - 0x57)
        if 0x50 <= b0 <= 0x57:
            regs = ["rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi"]
            reg_name = regs[b0 - 0x50]
            val = getattr(self.cpu, reg_name)
            self.push(val)
            self.execution_trace.append(f"0x{self.cpu.rip:x}: push {reg_name} (0x{val:x})")
            self.cpu.rip += 1
            self.steps_executed += 1
            return True

        # POP reg (0x58 - 0x5F)
        if 0x58 <= b0 <= 0x5F:
            regs = ["rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi"]
            reg_name = regs[b0 - 0x58]
            val = self.pop()
            setattr(self.cpu, reg_name, val)
            self.execution_trace.append(f"0x{self.cpu.rip:x}: pop {reg_name} (0x{val:x})")
            self.cpu.rip += 1
            self.steps_executed += 1
            return True

        # MOV reg, imm32 (0xB8 - 0xBF)
        if 0xB8 <= b0 <= 0xBF:
            if pc_offset + 5 > self.memory_size:
                self.halted = True
                self.halt_reason = "Truncated immediate instruction"
                return False
            regs = ["rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi"]
            reg_name = regs[b0 - 0xB8]
            imm = self._read_mem(self.cpu.rip + 1, 4)
            setattr(self.cpu, reg_name, imm)
            self.execution_trace.append(f"0x{self.cpu.rip:x}: mov {reg_name}, 0x{imm:x}")
            self.cpu.rip += 5
            self.steps_executed += 1
            return True

        # REX.W prefix (0x48) instructions:
        if b0 == 0x48 and pc_offset + 2 < self.memory_size:
            b1 = self.memory[pc_offset + 1]
            b2 = self.memory[pc_offset + 2]
            
            # XOR rax, rax (48 31 c0)
            if b1 == 0x31 and b2 == 0xC0:
                self.cpu.rax = 0
                self.execution_trace.append(f"0x{self.cpu.rip:x}: xor rax, rax")
                self.cpu.rip += 3
                self.steps_executed += 1
                return True
                
            # SUB rsp, imm8 (48 83 ec imm8)
            if b1 == 0x83 and b2 == 0xEC and pc_offset + 3 < self.memory_size:
                imm8 = self.memory[pc_offset + 3]
                self.cpu.rsp -= imm8
                self.execution_trace.append(f"0x{self.cpu.rip:x}: sub rsp, {imm8}")
                self.cpu.rip += 4
                self.steps_executed += 1
                return True
                
            # ADD rsp, imm8 (48 83 c4 imm8)
            if b1 == 0x83 and b2 == 0xC4 and pc_offset + 3 < self.memory_size:
                imm8 = self.memory[pc_offset + 3]
                self.cpu.rsp += imm8
                self.execution_trace.append(f"0x{self.cpu.rip:x}: add rsp, {imm8}")
                self.cpu.rip += 4
                self.steps_executed += 1
                return True

        # JMP rel8 (0xEB imm8)
        if b0 == 0xEB and pc_offset + 1 < self.memory_size:
            imm8 = self.memory[pc_offset + 1]
            if imm8 >= 128:
                imm8 -= 256
            target = self.cpu.rip + 2 + imm8
            self.execution_trace.append(f"0x{self.cpu.rip:x}: jmp 0x{target:x}")
            self.cpu.rip = target
            self.steps_executed += 1
            return True

        # Fallback: Safely stop emulation on unmodeled opcodes
        self.halted = True
        self.halt_reason = f"Unmodeled opcode 0x{b0:02x} encountered; safe halt"
        self.execution_trace.append(f"0x{self.cpu.rip:x}: unmodeled (0x{b0:02x})")
        return False

    def run(self, max_steps: Optional[int] = None) -> int:
        """Executes until halted or max step limit reached. Returns count of steps executed."""
        limit = max_steps or self.max_steps
        while not self.halted and self.steps_executed < limit:
            if not self.step():
                break
        return self.steps_executed
