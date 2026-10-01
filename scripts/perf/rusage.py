#!/usr/bin/env python3
"""proc_pid_rusage(RUSAGE_INFO_V2) 读取自有进程的 CPU 秒 / 磁盘写字节 / phys_footprint。
用法: rusage.py <pid>  → 一行 "cpu_s disk_written_B disk_read_B phys_footprint_B"
"""
import ctypes, sys

class RU(ctypes.Structure):
    _fields_ = [("uuid", ctypes.c_uint8 * 16)] + [(n, ctypes.c_uint64) for n in (
        "user_time", "system_time", "pkg_idle_wkups", "interrupt_wkups", "pageins",
        "wired_size", "resident_size", "phys_footprint", "proc_start_abstime",
        "proc_exit_abstime", "child_user_time", "child_system_time", "child_pkg_idle_wkups",
        "child_interrupt_wkups", "child_pageins", "child_elapsed_abstime",
        "diskio_bytesread", "diskio_byteswritten")]

class TB(ctypes.Structure):
    _fields_ = [("numer", ctypes.c_uint32), ("denom", ctypes.c_uint32)]

libc = ctypes.CDLL("/usr/lib/libSystem.B.dylib")
tb = TB(); libc.mach_timebase_info(ctypes.byref(tb))
ru = RU()
if libc.proc_pid_rusage(int(sys.argv[1]), 2, ctypes.byref(ru)) != 0:
    sys.exit(f"proc_pid_rusage failed for {sys.argv[1]}")
cpu = (ru.user_time + ru.system_time) * tb.numer / tb.denom / 1e9
print(f"{cpu:.4f} {ru.diskio_byteswritten} {ru.diskio_bytesread} {ru.phys_footprint}")
