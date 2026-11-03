"""In-memory PE and ELF structure recognition for forensic analysis."""

import struct
from dataclasses import dataclass
from typing import Optional, List, Dict

@dataclass
class InMemElfInfo:
    is_64bit: bool
    endianness: str
    machine_arch: int
    entry_point: int
    program_header_offset: int
    section_header_offset: int
    ph_count: int
    sh_count: int

@dataclass
class InMemPeInfo:
    is_64bit: bool
    machine: int
    entry_point: int
    image_base: int
    section_alignment: int
    file_alignment: int
    num_sections: int
    subsystem: int
    sections: List[Dict]

def detect_in_memory_binary(data: bytes, offset: int = 0) -> str:
    """Detects if memory contains an ELF or PE executable structure at offset."""
    if len(data) < offset + 4:
        return "UNKNOWN"
        
    magic = data[offset : offset + 4]
    if magic == b"\x7fELF":
        return "ELF"
    elif len(data) >= offset + 2 and data[offset : offset + 2] == b"MZ":
        # Check for PE signature at e_lfanew
        if len(data) >= offset + 0x3C + 4:
            e_lfanew = struct.unpack("<I", data[offset + 0x3C : offset + 0x40])[0]
            if len(data) >= offset + e_lfanew + 4:
                pe_sig = data[offset + e_lfanew : offset + e_lfanew + 4]
                if pe_sig == b"PE\x00\x00":
                    return "PE"
        return "DOS_HEADER"
    return "RAW"

def parse_in_memory_elf(data: bytes, offset: int = 0) -> Optional[InMemElfInfo]:
    """Parses ELF header from in-memory byte buffer."""
    if len(data) < offset + 64 or data[offset : offset + 4] != b"\x7fELF":
        return None
        
    ident = data[offset : offset + 16]
    is_64bit = ident[4] == 2
    is_le = ident[5] == 1
    endian = "<" if is_le else ">"
    
    try:
        if is_64bit:
            e_type, e_machine, e_version = struct.unpack(f"{endian}HHI", data[offset + 16 : offset + 24])
            e_entry = struct.unpack(f"{endian}Q", data[offset + 24 : offset + 32])[0]
            e_phoff = struct.unpack(f"{endian}Q", data[offset + 32 : offset + 40])[0]
            e_shoff = struct.unpack(f"{endian}Q", data[offset + 40 : offset + 48])[0]
            e_flags, e_ehsize, e_phentsize, e_phnum, e_shentsize, e_shnum, e_shstrndx = struct.unpack(
                f"{endian}IHHHHHH", data[offset + 48 : offset + 64]
            )
            return InMemElfInfo(
                is_64bit=True,
                endianness="LE" if is_le else "BE",
                machine_arch=e_machine,
                entry_point=e_entry,
                program_header_offset=e_phoff,
                section_header_offset=e_shoff,
                ph_count=e_phnum,
                sh_count=e_shnum,
            )
        else:
            e_type, e_machine, e_version = struct.unpack(f"{endian}HHI", data[offset + 16 : offset + 24])
            e_entry = struct.unpack(f"{endian}I", data[offset + 24 : offset + 28])[0]
            e_phoff = struct.unpack(f"{endian}I", data[offset + 28 : offset + 32])[0]
            e_shoff = struct.unpack(f"{endian}I", data[offset + 32 : offset + 36])[0]
            e_flags, e_ehsize, e_phentsize, e_phnum, e_shentsize, e_shnum = struct.unpack(
                f"{endian}IHHHHH", data[offset + 36 : offset + 50]
            )
            return InMemElfInfo(
                is_64bit=False,
                endianness="LE" if is_le else "BE",
                machine_arch=e_machine,
                entry_point=e_entry,
                program_header_offset=e_phoff,
                section_header_offset=e_shoff,
                ph_count=e_phnum,
                sh_count=e_shnum,
            )
    except Exception:
        return None

def parse_in_memory_pe(data: bytes, offset: int = 0) -> Optional[InMemPeInfo]:
    """Parses PE header and sections from in-memory byte buffer."""
    if len(data) < offset + 0x40 or data[offset : offset + 2] != b"MZ":
        return None
        
    try:
        e_lfanew = struct.unpack("<I", data[offset + 0x3C : offset + 0x40])[0]
        pe_offset = offset + e_lfanew
        
        if len(data) < pe_offset + 24 or data[pe_offset : pe_offset + 4] != b"PE\x00\x00":
            return None
            
        machine, num_sections, timedate, symtab, num_syms, opt_hdr_size, chars = struct.unpack(
            "<HHIIIHH", data[pe_offset + 4 : pe_offset + 24]
        )
        
        opt_hdr_offset = pe_offset + 24
        magic = struct.unpack("<H", data[opt_hdr_offset : opt_hdr_offset + 2])[0]
        is_64bit = (magic == 0x20B)  # PE32+
        
        if is_64bit:
            entry_point = struct.unpack("<I", data[opt_hdr_offset + 16 : opt_hdr_offset + 20])[0]
            image_base = struct.unpack("<Q", data[opt_hdr_offset + 24 : opt_hdr_offset + 32])[0]
            sec_align, file_align = struct.unpack("<II", data[opt_hdr_offset + 32 : opt_hdr_offset + 40])
            subsystem = struct.unpack("<H", data[opt_hdr_offset + 68 : opt_hdr_offset + 70])[0]
        else:
            entry_point = struct.unpack("<I", data[opt_hdr_offset + 16 : opt_hdr_offset + 20])[0]
            image_base = struct.unpack("<I", data[opt_hdr_offset + 28 : opt_hdr_offset + 32])[0]
            sec_align, file_align = struct.unpack("<II", data[opt_hdr_offset + 32 : opt_hdr_offset + 40])
            subsystem = struct.unpack("<H", data[opt_hdr_offset + 68 : opt_hdr_offset + 70])[0]
            
        sections = []
        sec_table_offset = opt_hdr_offset + opt_hdr_size
        for i in range(num_sections):
            curr_sec = sec_table_offset + i * 40
            if len(data) >= curr_sec + 40:
                name = data[curr_sec : curr_sec + 8].rstrip(b"\x00").decode("ascii", errors="replace")
                vsize, vaddr, raw_size, raw_offset = struct.unpack("<IIII", data[curr_sec + 8 : curr_sec + 24])
                chars = struct.unpack("<I", data[curr_sec + 36 : curr_sec + 40])[0]
                sections.append({
                    "name": name,
                    "virtual_address": vaddr,
                    "virtual_size": vsize,
                    "raw_size": raw_size,
                    "raw_offset": raw_offset,
                    "characteristics": chars,
                    "is_executable": bool(chars & 0x20000000),
                    "is_readable": bool(chars & 0x40000000),
                    "is_writable": bool(chars & 0x80000000),
                })
                
        return InMemPeInfo(
            is_64bit=is_64bit,
            machine=machine,
            entry_point=entry_point,
            image_base=image_base,
            section_alignment=sec_align,
            file_alignment=file_align,
            num_sections=num_sections,
            subsystem=subsystem,
            sections=sections,
        )
    except Exception:
        return None
