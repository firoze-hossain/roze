import unittest
from roze_forensics.disasm import DisassembledInstruction
from roze_forensics.control_flow import ControlFlowAuditor

class TestControlFlowAuditor(unittest.TestCase):
    def test_stack_pivot_detection(self):
        auditor = ControlFlowAuditor()
        # Create sequence: pop rsp (5c); ret (c3)
        insns = [
            DisassembledInstruction(
                address=0x401000,
                mnemonic="pop",
                op_str="rsp",
                size=1,
                raw_bytes=b"\x5c",
                category="STACK",
            ),
            DisassembledInstruction(
                address=0x401001,
                mnemonic="ret",
                op_str="",
                size=1,
                raw_bytes=b"\xc3",
                category="CONTROL_FLOW",
            ),
        ]
        anomalies = auditor.audit_instructions(insns)
        types = [a.anomaly_type for a in anomalies]
        self.assertIn("STACK_PIVOT_SEQUENCE_DETECTED", types)

    def test_high_density_control_flow(self):
        auditor = ControlFlowAuditor(branch_density_threshold=0.3)
        # Create sequence of 10 instructions where 5 are control flow
        insns = []
        for i in range(5):
            insns.append(
                DisassembledInstruction(
                    address=0x401000 + i * 2,
                    mnemonic="ret",
                    op_str="",
                    size=1,
                    raw_bytes=b"\xc3",
                    category="CONTROL_FLOW",
                )
            )
        for i in range(5):
            insns.append(
                DisassembledInstruction(
                    address=0x401010 + i * 2,
                    mnemonic="nop",
                    op_str="",
                    size=1,
                    raw_bytes=b"\x90",
                    category="MISC",
                )
            )
        anomalies = auditor.audit_instructions(insns)
        types = [a.anomaly_type for a in anomalies]
        self.assertIn("HIGH_DENSITY_CONTROL_FLOW", types)

if __name__ == "__main__":
    unittest.main()
