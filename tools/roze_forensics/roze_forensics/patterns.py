"""Pattern searching engine with wildcard support, string scanning, and YARA integration."""

import re
from typing import List, Tuple, Optional, Dict
from dataclasses import dataclass
from .logger import logger

try:
    import yara
    HAS_YARA = True
except ImportError:
    HAS_YARA = False

@dataclass
class PatternMatch:
    offset: int
    matched_bytes: bytes
    pattern_name: str = ""
    description: str = ""

@dataclass
class CompiledWildcardPattern:
    raw_pattern: str
    regex: re.Pattern

def compile_hex_pattern(pattern_str: str, name: str = "") -> CompiledWildcardPattern:
    """Compiles a hex pattern with wildcards (e.g. '48 89 ?? ?? c3') into a regex."""
    clean_tokens = pattern_str.strip().split()
    regex_parts = []
    
    for token in clean_tokens:
        token = token.strip()
        if token in ("?", "??"):
            regex_parts.append(b".")
        else:
            byte_val = bytes.fromhex(token)
            regex_parts.append(re.escape(byte_val))
            
    compiled_re = re.compile(b"".join(regex_parts), re.DOTALL)
    return CompiledWildcardPattern(raw_pattern=pattern_str, regex=compiled_re)

def find_hex_pattern(data: bytes, pattern: CompiledWildcardPattern) -> List[PatternMatch]:
    """Finds all occurrences of a compiled wildcard pattern in data."""
    matches = []
    for m in pattern.regex.finditer(data):
        matches.append(
            PatternMatch(
                offset=m.start(),
                matched_bytes=m.group(),
                pattern_name=pattern.raw_pattern,
            )
        )
    return matches

def search_strings(data: bytes, min_length: int = 4) -> List[Tuple[int, str]]:
    """Extracts printable ASCII and UTF-8 strings with their offsets."""
    results = []
    pattern = re.compile(rb"[\x20-\x7e]{" + str(min_length).encode() + rb",}")
    for m in pattern.finditer(data):
        try:
            s = m.group().decode("ascii")
            results.append((m.start(), s))
        except Exception:
            pass
    return results

def search_wide_strings(data: bytes, min_length: int = 4) -> List[Tuple[int, str]]:
    """Extracts UTF-16LE wide strings (common in Windows memory dumps)."""
    results = []
    pattern = re.compile(rb"(?:[\x20-\x7e]\x00){" + str(min_length).encode() + rb",}")
    for m in pattern.finditer(data):
        try:
            s = m.group().decode("utf-16le")
            results.append((m.start(), s))
        except Exception:
            pass
    return results

class YaraScanner:
    """YARA signature scanning wrapper with graceful fallback."""

    def __init__(self, rule_file_or_string: Optional[str] = None):
        self.rules = None
        self.enabled = False
        if not HAS_YARA:
            logger.info("YARA python package not installed; native YARA scanning is disabled.")
            return
            
        if rule_file_or_string:
            try:
                if "\n" in rule_file_or_string or "rule " in rule_file_or_string:
                    self.rules = yara.compile(source=rule_file_or_string)
                else:
                    self.rules = yara.compile(filepath=rule_file_or_string)
                self.enabled = True
            except Exception as e:
                logger.error(f"Failed to compile YARA rules: {e}")

    def scan_bytes(self, data: bytes) -> List[Dict]:
        """Scans byte data against compiled YARA rules."""
        if not self.enabled or not self.rules:
            return []
            
        try:
            matches = self.rules.match(data=data)
            results = []
            for m in matches:
                strings_matched = []
                for s in m.strings:
                    # s is a tuple: (offset, identifier, data)
                    strings_matched.append({
                        "offset": s[0],
                        "identifier": s[1],
                        "data": s[2].hex(),
                    })
                results.append({
                    "rule": m.rule,
                    "tags": m.tags,
                    "meta": m.meta,
                    "strings": strings_matched,
                })
            return results
        except Exception as e:
            logger.error(f"Error during YARA scan: {e}")
            return []
