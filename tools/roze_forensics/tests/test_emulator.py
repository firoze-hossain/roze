import unittest
from roze_forensics.emulator import SandboxedCpuEmulator

class TestSandboxedCpuEmulator(unittest.TestCase):
    def test_nop_and_mov_and_ret(self):
        emu = SandboxedCpuEmulator(memory_size=4096, base_address=0x400000)
        # Machine code:
        # 90          nop
        # b8 2a 00 00 00  mov eax, 42
        # 50          push rax
        # 5b          pop rbx
        # c3          ret
        code = b"\x90\xb8\x2a\x00\x00\x00\x50\x5b\xc3"
        emu.load_bytes(code, target_address=0x400000)
        
        # Setup fake return address on stack
        emu.push(0x400050)
        
        steps = emu.run(max_steps=10)
        self.assertEqual(emu.cpu.rax, 42)
        self.assertEqual(emu.cpu.rbx, 42)
        self.assertEqual(emu.cpu.rip, 0x400050)

    def test_bounds_checking_halt(self):
        emu = SandboxedCpuEmulator(memory_size=1024, base_address=0x400000)
        # Attempt to jump way outside memory bounds: jmp 0x800000 (eb fe ... or unmodeled/bounds)
        emu.cpu.rip = 0x900000
        result = emu.step()
        self.assertFalse(result)
        self.assertTrue(emu.halted)
        self.assertIn("out of bounds", emu.halt_reason)

    def test_int3_and_hlt_safely_halt(self):
        emu = SandboxedCpuEmulator(memory_size=1024, base_address=0x400000)
        emu.load_bytes(b"\xcc", target_address=0x400000)
        result = emu.step()
        self.assertFalse(result)
        self.assertTrue(emu.halted)
        self.assertIn("int3", emu.halt_reason)

if __name__ == "__main__":
    unittest.main()
