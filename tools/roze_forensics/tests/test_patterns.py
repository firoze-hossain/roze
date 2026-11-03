import unittest
from roze_forensics.patterns import (
    compile_hex_pattern,
    find_hex_pattern,
    search_strings,
    search_wide_strings,
)

class TestPatternMatching(unittest.TestCase):
    def test_exact_hex_pattern(self):
        pat = compile_hex_pattern("48 89 e5")
        data = b"\x90\x90\x48\x89\xe5\x90\x90"
        matches = find_hex_pattern(data, pat)
        self.assertEqual(len(matches), 1)
        self.assertEqual(matches[0].offset, 2)
        self.assertEqual(matches[0].matched_bytes, b"\x48\x89\xe5")

    def test_wildcard_hex_pattern(self):
        # Match call instruction: e8 ?? ?? ?? ?? followed by test rax, rax (48 85 c0)
        pat = compile_hex_pattern("e8 ?? ?? ?? ?? 48 85 c0")
        data = b"\x90\xe8\x12\x34\x56\x78\x48\x85\xc0\xc3"
        matches = find_hex_pattern(data, pat)
        self.assertEqual(len(matches), 1)
        self.assertEqual(matches[0].offset, 1)
        self.assertEqual(matches[0].matched_bytes, b"\xe8\x12\x34\x56\x78\x48\x85\xc0")

    def test_ascii_and_wide_strings(self):
        data = b"prefix\x00TargetString\x00mid" + "WideTarget".encode("utf-16le") + b"\x00\x00"
        ascii_strs = [s[1] for s in search_strings(data, min_length=6)]
        self.assertIn("prefix", ascii_strs)
        self.assertIn("TargetString", ascii_strs)
        
        wide_strs = [s[1] for s in search_wide_strings(data, min_length=6)]
        self.assertIn("WideTarget", wide_strs)

if __name__ == "__main__":
    unittest.main()
