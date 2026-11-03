"""Defensive control-flow anomaly detection & incident reporting."""

from dataclasses import dataclass
from typing import List, Dict, Optional
from .disasm import DisassembledInstruction, disassemble_buffer
from .logger import logger

@dataclass
class ControlFlowAnomaly:
    anomaly_type: str
    severity: str
    address: int
    instruction_sequence: str
    description: str

class ControlFlowAuditor:
    """Defensive auditor analyzing disassembled instruction streams for anomalous control-flow patterns."""

    def __init__(self, branch_density_threshold: float = 0.35):
        self.branch_density_threshold = branch_density_threshold

    def audit_instructions(self, instructions: List[DisassembledInstruction]) -> List[ControlFlowAnomaly]:
        """Scans a disassembled instruction stream for anomalous control-flow characteristics."""
        anomalies = []
        if not instructions:
            return anomalies
            
        total_count = len(instructions)
        control_flow_count = sum(1 for i in instructions if i.category in ("CONTROL_FLOW", "SYSTEM"))
        
        # 1. High-Density Control Flow Anomaly
        if total_count >= 10:
            density = control_flow_count / total_count
            if density >= self.branch_density_threshold:
                anomalies.append(
                    ControlFlowAnomaly(
                        anomaly_type="HIGH_DENSITY_CONTROL_FLOW",
                        severity="MEDIUM",
                        address=instructions[0].address,
                        instruction_sequence=f"{control_flow_count}/{total_count} instructions ({density*100:.1f}%)",
                        description="Abnormally high proportion of control-flow and system transfer instructions, characteristic of staging code or anomalous dispatchers.",
                    )
                )

        # 2. Sequential Stack Pivot Indicator
        # Checks for sequences where stack pointer adjustments precede return/indirect jump
        for idx in range(len(instructions) - 1):
            curr = instructions[idx]
            nxt = instructions[idx + 1]
            
            # Check for stack pointer manipulation (e.g. pop rsp, mov rsp, add rsp)
            is_sp_mod = False
            if curr.mnemonic in ("pop", "mov", "add", "sub", "xchg"):
                if "rsp" in curr.op_str.lower() or "esp" in curr.op_str.lower():
                    is_sp_mod = True
                    
            if is_sp_mod and nxt.mnemonic in ("ret", "retn", "jmp"):
                anomalies.append(
                    ControlFlowAnomaly(
                        anomaly_type="STACK_PIVOT_SEQUENCE_DETECTED",
                        severity="HIGH",
                        address=curr.address,
                        instruction_sequence=f"{curr.mnemonic} {curr.op_str} ; {nxt.mnemonic} {nxt.op_str}",
                        description="Instruction sequence alters stack pointer immediately prior to indirect transfer (defensive pivot detection).",
                    )
                )

        # 3. Repeated Return Cluster Anomaly
        ret_count = 0
        for i in instructions:
            if i.mnemonic.startswith("ret"):
                ret_count += 1
        if total_count >= 15 and (ret_count / total_count) > 0.20:
            anomalies.append(
                ControlFlowAnomaly(
                    anomaly_type="CONCENTRATED_RETURN_CLUSTER",
                    severity="MEDIUM",
                    address=instructions[0].address,
                    instruction_sequence=f"{ret_count} returns in {total_count} instructions",
                    description="Concentration of isolated return opcodes without intervening function prologues.",
                )
            )

        return anomalies
