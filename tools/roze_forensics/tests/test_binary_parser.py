import unittest
from roze_forensics.binary_parser import (
    detect_in_memory_binary,
    parse_in_memory_elf,
    parse_in_memory_pe,
)

class TestBinaryParser(unittest.TestCase):
    def test_detect_elf(self):
        elf_header = (
            b"\x7fELF"  # magic
            + b"\x02"   # 64-bit
            + b"\x01"   # Little-endian
            + b"\x01"   # version
            + b"\x00" * 9  # padding
            + b"\x02\x00"  # ET_EXEC
            + b"\x3e\x00"  # x86_64
            + b"\x01\x00\x00\x00"  # version
            + b"\x00\x10\x40\x00\x00\x00\x00\x00"  # entry point: 0x401000
            + b"\x40\x00\x00\x00\x00\x00\x00\x00"  # phoff: 64
            + b"\x00\x00\x00\x00\x00\x00\x00\x00"  # shoff: 0
            + b"\x00\x00\x00\x00"  # flags
            + b"\x40\x00"  # ehsize: 64
            + b"\x38\x00"  # phentsize: 56
            + b"\x02\x00"  # phnum: 2
            + b"\x40\x00"  # shentsize: 64
            + b"\x00\x00"  # shnum: 0
            + b"\x00\x00"  # shstrndx: 0
        )
        self.assertEqual(detect_in_memory_binary(elf_header), "ELF")
        elf_info = parse_in_memory_elf(elf_header)
        self.assertIsNotNone(elf_info)
        self.assertTrue(elf_info.is_64bit)
        self.assertEqual(elf_info.endianness, "LE")
        self.assertEqual(elf_info.entry_point, 0x401000)
        self.assertEqual(elf_info.ph_count, 2)

    def test_detect_pe(self):
        # Create minimal valid PE structure
        pe_data = bytearray(512)
        pe_data[0:2] = b"MZ"
        # e_lfanew at 0x3C = 0x80
        pe_data[0x3C:0x40] = (0x80).to_bytes(4, "little")
        # PE signature at 0x80
        pe_data[0x80:0x84] = b"PE\x00\x00"
        # Machine x86_64 = 0x8664
        pe_data[0x84:0x86] = (0x8664).to_bytes(2, "little")
        # Num sections = 1
        pe_data[0x86:0x88] = (1).to_bytes(2, "little")
        # Size of optional header = 240
        pe_data[0x94:0x96] = (240).to_bytes(2, "little")
        # Optional header magic PE32+ (0x20B)
        pe_data[0x98:0x9A] = (0x20B).to_bytes(2, "little")
        # Entry point RVA = 0x1000
        pe_data[0x98 + 16 : 0x98 + 20] = (0x1000).to_bytes(4, "little")
        # Image base = 0x140000000
        pe_data[0x98 + 24 : 0x98 + 32] = (0x140000000).to_bytes(8, "little")
        
        self.assertEqual(detect_in_memory_binary(bytes(pe_data)), "PE")
        pe_info = parse_in_memory_pe(bytes(pe_data))
        self.assertIsNotNone(pe_info)
        self.assertTrue(pe_info.is_64bit)
        self.assertEqual(pe_info.entry_point, 0x1000)
        self.assertEqual(pe_info.image_base, 0x140000000)

if __name__ == "__main__":
    unittest.main()
