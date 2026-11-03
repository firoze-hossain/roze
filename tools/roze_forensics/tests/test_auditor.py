import unittest
from roze_forensics.memory_reader import MemoryRegion
from roze_forensics.auditor import MemoryAuditor
from roze_forensics.config import ForensicsConfig

class TestMemoryAuditor(unittest.TestCase):
    def setUp(self):
        self.auditor = MemoryAuditor(ForensicsConfig())

    def test_rwx_violation_flagged(self):
        region = MemoryRegion(
            start_address=0x100000,
            end_address=0x102000,
            permissions="rwxp",
            pathname="[heap]",
            is_readable=True,
            is_writable=True,
            is_executable=True,
            is_anonymous=True,
        )
        anomalies = self.auditor.audit_region(region)
        types = [a.anomaly_type for a in anomalies]
        self.assertIn("RWX_PERMISSION_VIOLATION", types)
        self.assertIn("EXECUTABLE_HEAP_DETECTED", types)

    def test_unbacked_executable_memory_flagged(self):
        region = MemoryRegion(
            start_address=0x7f000000,
            end_address=0x7f001000,
            permissions="r-xp",
            pathname="",  # Anonymous
            is_readable=True,
            is_writable=False,
            is_executable=True,
            is_anonymous=True,
        )
        anomalies = self.auditor.audit_region(region)
        types = [a.anomaly_type for a in anomalies]
        self.assertIn("UNBACKED_EXECUTABLE_MEMORY", types)

    def test_clean_file_backed_region_not_flagged(self):
        region = MemoryRegion(
            start_address=0x400000,
            end_address=0x450000,
            permissions="r-xp",
            pathname="/usr/bin/bash",
            is_readable=True,
            is_writable=False,
            is_executable=True,
            is_anonymous=False,
        )
        anomalies = self.auditor.audit_region(region)
        self.assertEqual(len(anomalies), 0)

if __name__ == "__main__":
    unittest.main()
