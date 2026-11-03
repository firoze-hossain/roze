import unittest
import os
import tempfile
from roze_forensics.dump_parser import MemoryDumpParser
from roze_forensics.auditor import MemoryAuditor
from roze_forensics.config import ForensicsConfig
from roze_forensics.patterns import compile_hex_pattern, find_hex_pattern
from roze_forensics.reporter import ForensicsAuditReport

class TestForensicsIntegration(unittest.TestCase):
    def test_full_pipeline_on_synthetic_dump(self):
        # Create a synthetic memory dump with embedded patterns and PE structure
        with tempfile.NamedTemporaryFile(delete=False) as f:
            dump_path = f.name
            
            # Fill with 8KB
            buffer = bytearray(8192)
            # Add an ASCII marker string
            buffer[100:114] = b"ForensicTarget"
            # Add a known instruction sequence: mov eax, 10; ret
            buffer[200:206] = b"\xb8\x0a\x00\x00\x00\xc3"
            
            f.write(buffer)

        try:
            parser = MemoryDumpParser(dump_path, base_virtual_address=0x1000000)
            regions = parser.get_memory_regions()
            self.assertEqual(len(regions), 1)
            
            auditor = MemoryAuditor(ForensicsConfig())
            anomalies = auditor.audit_all_regions(regions, reader=parser)
            
            # Search pattern
            pat = compile_hex_pattern("b8 0a 00 00 00 c3")
            data = parser.read_bytes(0x1000000, 8192)
            matches = find_hex_pattern(data, pat)
            self.assertEqual(len(matches), 1)
            self.assertEqual(matches[0].offset, 200)

            # Generate report
            report = ForensicsAuditReport(
                target_identifier=dump_path,
                target_type="MEMORY_DUMP",
                total_regions_enumerated=len(regions),
                total_memory_bytes=parser.file_size,
                executable_regions_count=1,
                writable_executable_count=1,
                anomalies=anomalies,
                pattern_matches=[{"pattern": "b8 0a 00 00 00 c3", "offset": 200}],
            )
            
            report_dict = report.to_dict()
            self.assertEqual(report_dict["statistics"]["total_regions"], 1)
            self.assertEqual(len(report_dict["pattern_matches"]), 1)
            
            table_str = report.format_ascii_table()
            self.assertIn("ROZE MEMORY FORENSICS", table_str)
            
            parser.close()
        finally:
            if os.path.exists(dump_path):
                os.unlink(dump_path)

if __name__ == "__main__":
    unittest.main()
