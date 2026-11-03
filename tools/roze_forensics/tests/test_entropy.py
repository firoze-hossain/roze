import unittest
import math
from roze_forensics.entropy import calculate_shannon_entropy, analyze_entropy_blocks, is_high_entropy

class TestEntropyAnalysis(unittest.TestCase):
    def test_zero_entropy_for_empty_and_uniform_buffer(self):
        self.assertEqual(calculate_shannon_entropy(b""), 0.0)
        # All zeros has 0 entropy
        zeros = b"\x00" * 1024
        self.assertEqual(calculate_shannon_entropy(zeros), 0.0)

    def test_maximum_entropy_for_uniform_distribution(self):
        # 256 unique bytes evenly distributed has exactly 8.0 bits entropy
        data = bytes(range(256)) * 4
        entropy = calculate_shannon_entropy(data)
        self.assertAlmostEqual(entropy, 8.0, places=4)

    def test_entropy_blocks(self):
        data = (b"\x00" * 512) + bytes(range(256)) * 2
        blocks = analyze_entropy_blocks(data, block_size=512)
        self.assertEqual(len(blocks), 2)
        self.assertEqual(blocks[0][0], 0)
        self.assertEqual(blocks[0][1], 0.0)
        self.assertEqual(blocks[1][0], 512)
        self.assertAlmostEqual(blocks[1][1], 8.0, places=4)

    def test_is_high_entropy(self):
        low_data = b"Hello, this is standard ascii text without compression."
        high_data = bytes(range(256)) * 4
        self.assertFalse(is_high_entropy(low_data, threshold=7.0))
        self.assertTrue(is_high_entropy(high_data, threshold=7.0))

if __name__ == "__main__":
    unittest.main()
