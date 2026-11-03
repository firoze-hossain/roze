"""Offline memory dump loader and virtual address mapper for forensics investigation."""

import os
import struct
from dataclasses import dataclass
from typing import List, Optional, Dict
from .memory_reader import MemoryRegion
from .logger import logger

@dataclass
class DumpSegment:
    virtual_address: int
    size: int
    file_offset: int
    permissions: str
    is_readable: bool = True
    is_writable: bool = False
    is_executable: bool = False

class MemoryDumpParser:
    """Safe offline memory dump parser and address space mapper."""

    def __init__(self, dump_path: str, base_virtual_address: int = 0x400000):
        self.dump_path = dump_path
        self.base_address = base_virtual_address
        self.file_size = os.path.getsize(dump_path) if os.path.exists(dump_path) else 0
        self.segments: List[DumpSegment] = []
        self._file = None
        self._init_dump()

    def _init_dump(self):
        if not os.path.exists(self.dump_path):
            raise FileNotFoundError(f"Memory dump file not found: {self.dump_path}")
            
        self._file = open(self.dump_path, "rb")
        
        # Check if dump is an ELF Core dump (0x7F, 'E', 'L', 'F')
        header_bytes = self._file.read(64)
        self._file.seek(0)
        
        if header_bytes.startswith(b"\x7fELF"):
            self._parse_elf_core_headers()
        else:
            # Treat as flat raw memory image loaded at base_address
            self.segments.append(
                DumpSegment(
                    virtual_address=self.base_address,
                    size=self.file_size,
                    file_offset=0,
                    permissions="rwxp",
                    is_readable=True,
                    is_writable=True,
                    is_executable=True,
                )
            )

    def _parse_elf_core_headers(self):
        """Extracts PT_LOAD segments from an ELF core dump."""
        try:
            self._file.seek(0)
            ident = self._file.read(16)
            is_64bit = ident[4] == 2
            
            if is_64bit:
                # 64-bit ELF Header
                self._file.seek(32)
                e_phoff = struct.unpack("<Q", self._file.read(8))[0]
                self._file.seek(54)
                e_phentsize = struct.unpack("<H", self._file.read(2))[0]
                e_phnum = struct.unpack("<H", self._file.read(2))[0]
                
                for i in range(e_phnum):
                    self._file.seek(e_phoff + i * e_phentsize)
                    p_type, p_flags = struct.unpack("<II", self._file.read(8))
                    p_offset, p_vaddr, p_paddr, p_filesz, p_memsz = struct.unpack("<QQQQQ", self._file.read(40))
                    
                    # PT_LOAD = 1
                    if p_type == 1 and p_filesz > 0:
                        is_r = bool(p_flags & 4)
                        is_w = bool(p_flags & 2)
                        is_x = bool(p_flags & 1)
                        perms = ("r" if is_r else "-") + ("w" if is_w else "-") + ("x" if is_x else "-") + "p"
                        self.segments.append(
                            DumpSegment(
                                virtual_address=p_vaddr,
                                size=p_filesz,
                                file_offset=p_offset,
                                permissions=perms,
                                is_readable=is_r,
                                is_writable=is_w,
                                is_executable=is_x,
                            )
                        )
        except Exception as e:
            logger.warning(f"Failed to parse ELF core headers, falling back to flat raw dump: {e}")
            self.segments = [
                DumpSegment(
                    virtual_address=self.base_address,
                    size=self.file_size,
                    file_offset=0,
                    permissions="rwxp",
                    is_readable=True,
                    is_writable=True,
                    is_executable=True,
                )
            ]

    def close(self):
        if self._file:
            try:
                self._file.close()
            except Exception:
                pass
            self._file = None

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def get_memory_regions(self) -> List[MemoryRegion]:
        """Converts dump segments to standardized MemoryRegion instances."""
        regions = []
        for seg in self.segments:
            reg = MemoryRegion(
                start_address=seg.virtual_address,
                end_address=seg.virtual_address + seg.size,
                permissions=seg.permissions,
                offset=seg.file_offset,
                pathname=f"[dump_seg_0x{seg.virtual_address:x}]",
                is_readable=seg.is_readable,
                is_writable=seg.is_writable,
                is_executable=seg.is_executable,
                is_shared=False,
                is_anonymous=False,
            )
            regions.append(reg)
        return regions

    def read_bytes(self, address: int, length: int) -> bytes:
        """Reads bytes at virtual address from corresponding segment."""
        if not self._file or length <= 0:
            return b""
            
        for seg in self.segments:
            if seg.virtual_address <= address < seg.virtual_address + seg.size:
                offset_in_seg = address - seg.virtual_address
                available = seg.size - offset_in_seg
                read_len = min(length, available)
                self._file.seek(seg.file_offset + offset_in_seg)
                return self._file.read(read_len)
        return b""
