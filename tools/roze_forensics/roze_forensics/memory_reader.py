"""Cross-platform process memory inspection & mapping abstractions for Linux and Windows."""

import os
import sys
import platform
import ctypes
from dataclasses import dataclass
from typing import List, Optional, Tuple
from .logger import logger

@dataclass
class MemoryRegion:
    start_address: int
    end_address: int
    permissions: str
    offset: int = 0
    device: str = ""
    inode: int = 0
    pathname: str = ""
    is_readable: bool = False
    is_writable: bool = False
    is_executable: bool = False
    is_shared: bool = False
    is_anonymous: bool = True

    @property
    def size(self) -> int:
        return self.end_address - self.start_address

    @property
    def is_rwx(self) -> bool:
        return self.is_readable and self.is_writable and self.is_executable

    def to_dict(self) -> dict:
        return {
            "start_address": f"0x{self.start_address:016x}",
            "end_address": f"0x{self.end_address:016x}",
            "size": self.size,
            "permissions": self.permissions,
            "offset": f"0x{self.offset:x}",
            "pathname": self.pathname,
            "is_rwx": self.is_rwx,
            "is_anonymous": self.is_anonymous,
        }

class ProcessMemoryReader:
    """Read-only process memory accessor supporting Linux and Windows."""
    
    def __init__(self, pid: int):
        self.pid = pid
        self.os_type = platform.system().lower()
        self.is_linux = "linux" in self.os_type
        self.is_windows = "windows" in self.os_type
        self._mem_file = None
        self._process_handle = None
        self._init_reader()

    def _init_reader(self):
        if self.is_linux:
            mem_path = f"/proc/{self.pid}/mem"
            if os.path.exists(mem_path):
                try:
                    self._mem_file = open(mem_path, "rb", buffering=0)
                except PermissionError:
                    logger.warning(f"Insufficient permissions to read {mem_path}. Run with appropriate capabilities (CAP_SYS_PTRACE).")
                except Exception as e:
                    logger.warning(f"Could not open {mem_path}: {e}")
        elif self.is_windows:
            self._init_windows_handle()

    def _init_windows_handle(self):
        try:
            # PROCESS_VM_READ (0x0010) | PROCESS_QUERY_INFORMATION (0x0400)
            PROCESS_VM_READ = 0x0010
            PROCESS_QUERY_INFORMATION = 0x0400
            kernel32 = ctypes.windll.kernel32
            self._process_handle = kernel32.OpenProcess(
                PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, False, self.pid
            )
            if not self._process_handle:
                err = kernel32.GetLastError()
                logger.warning(f"Failed to open Windows process {self.pid}: error {err}")
        except Exception as e:
            logger.warning(f"Windows API initialization failed: {e}")

    def close(self):
        if self._mem_file:
            try:
                self._mem_file.close()
            except Exception:
                pass
            self._mem_file = None
        if self._process_handle and self.is_windows:
            try:
                ctypes.windll.kernel32.CloseHandle(self._process_handle)
            except Exception:
                pass
            self._process_handle = None

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def enumerate_regions(self) -> List[MemoryRegion]:
        """Enumerates virtual memory regions for the target process."""
        if self.is_linux:
            return self._enumerate_linux_regions()
        elif self.is_windows:
            return self._enumerate_windows_regions()
        return []

    def _enumerate_linux_regions(self) -> List[MemoryRegion]:
        maps_path = f"/proc/{self.pid}/maps"
        if not os.path.exists(maps_path):
            logger.error(f"Target process map file does not exist: {maps_path}")
            return []
            
        regions = []
        try:
            with open(maps_path, "r", encoding="utf-8", errors="replace") as f:
                for line in f:
                    line = line.strip()
                    if not line:
                        continue
                    parts = line.split(maxsplit=5)
                    if len(parts) < 5:
                        continue
                        
                    addr_range = parts[0].split("-")
                    if len(addr_range) != 2:
                        continue
                    start_addr = int(addr_range[0], 16)
                    end_addr = int(addr_range[1], 16)
                    perms = parts[1]
                    offset = int(parts[2], 16)
                    dev = parts[3]
                    inode = int(parts[4])
                    pathname = parts[5] if len(parts) > 5 else ""
                    
                    is_r = perms.startswith("r")
                    is_w = len(perms) > 1 and perms[1] == "w"
                    is_x = len(perms) > 2 and perms[2] == "x"
                    is_s = len(perms) > 3 and perms[3] == "s"
                    is_anon = (pathname == "" or pathname.startswith("["))
                    
                    region = MemoryRegion(
                        start_address=start_addr,
                        end_address=end_addr,
                        permissions=perms,
                        offset=offset,
                        device=dev,
                        inode=inode,
                        pathname=pathname,
                        is_readable=is_r,
                        is_writable=is_w,
                        is_executable=is_x,
                        is_shared=is_s,
                        is_anonymous=is_anon,
                    )
                    regions.append(region)
        except Exception as e:
            logger.error(f"Failed to parse {maps_path}: {e}")
        return regions

    def _enumerate_windows_regions(self) -> List[MemoryRegion]:
        regions = []
        if not self._process_handle:
            return regions
            
        try:
            kernel32 = ctypes.windll.kernel32
            
            class MEMORY_BASIC_INFORMATION(ctypes.Structure):
                _fields_ = [
                    ("BaseAddress", ctypes.c_void_p),
                    ("AllocationBase", ctypes.c_void_p),
                    ("AllocationProtect", ctypes.c_ulong),
                    ("RegionSize", ctypes.c_size_t),
                    ("State", ctypes.c_ulong),
                    ("Protect", ctypes.c_ulong),
                    ("Type", ctypes.c_ulong),
                ]
                
            mbi = MEMORY_BASIC_INFORMATION()
            address = 0
            # MEM_COMMIT = 0x1000
            MEM_COMMIT = 0x1000
            
            # Protection flags
            PAGE_NOACCESS = 0x01
            PAGE_READONLY = 0x02
            PAGE_READWRITE = 0x04
            PAGE_WRITECOPY = 0x08
            PAGE_EXECUTE = 0x10
            PAGE_EXECUTE_READ = 0x20
            PAGE_EXECUTE_READWRITE = 0x40
            PAGE_EXECUTE_WRITECOPY = 0x80
            PAGE_GUARD = 0x100
            
            while kernel32.VirtualQueryEx(
                self._process_handle,
                ctypes.c_void_p(address),
                ctypes.byref(mbi),
                ctypes.sizeof(mbi)
            ):
                if mbi.State == MEM_COMMIT and not (mbi.Protect & PAGE_GUARD):
                    prot = mbi.Protect
                    is_r = bool(prot & (PAGE_READONLY | PAGE_READWRITE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE))
                    is_w = bool(prot & (PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY))
                    is_x = bool(prot & (PAGE_EXECUTE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY))
                    
                    perms_str = ("r" if is_r else "-") + ("w" if is_w else "-") + ("x" if is_x else "-") + "p"
                    base = mbi.BaseAddress or 0
                    size = mbi.RegionSize
                    
                    region = MemoryRegion(
                        start_address=base,
                        end_address=base + size,
                        permissions=perms_str,
                        offset=0,
                        pathname="[win32_committed]",
                        is_readable=is_r,
                        is_writable=is_w,
                        is_executable=is_x,
                        is_shared=False,
                        is_anonymous=True,
                    )
                    regions.append(region)
                address = (mbi.BaseAddress or 0) + mbi.RegionSize
                if address >= 0x7FFFFFFFFFFFFFFF:
                    break
        except Exception as e:
            logger.error(f"VirtualQueryEx enumeration failed: {e}")
        return regions

    def read_bytes(self, address: int, length: int) -> bytes:
        """Safely reads raw bytes from the target address."""
        if length <= 0:
            return b""
            
        if self.is_linux and self._mem_file:
            try:
                self._mem_file.seek(address)
                return self._mem_file.read(length)
            except Exception:
                return b""
                
        elif self.is_windows and self._process_handle:
            try:
                kernel32 = ctypes.windll.kernel32
                buffer = ctypes.create_string_buffer(length)
                bytes_read = ctypes.c_size_t(0)
                if kernel32.ReadProcessMemory(
                    self._process_handle,
                    ctypes.c_void_p(address),
                    buffer,
                    length,
                    ctypes.byref(bytes_read)
                ):
                    return buffer.raw[:bytes_read.value]
            except Exception:
                return b""
        return b""
