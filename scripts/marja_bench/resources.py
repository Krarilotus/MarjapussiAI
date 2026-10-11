"""One bounded Windows process tree, including Cargo and all game workers."""

import ctypes as c
import ctypes.wintypes as w
import os
from pathlib import Path


class RunLock:
    def __init__(self, path):
        self.path = Path(path)

    def __enter__(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.file = self.path.open("a+b")
        self.file.seek(0)
        try:
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(self.file.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(self.file, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            self.file.close()
            raise RuntimeError(f"Another Marja validation job is running ({self.path}).") from None
        return self

    def __exit__(self, *_):
        self.file.close()


class Basic(c.Structure):
    _fields_ = [("process_time", c.c_longlong), ("job_time", c.c_longlong),
                ("flags", w.DWORD), ("min_ws", c.c_size_t), ("max_ws", c.c_size_t),
                ("active", w.DWORD), ("affinity", c.c_size_t),
                ("priority", w.DWORD), ("scheduling", w.DWORD)]


class Io(c.Structure):
    _fields_ = [(name, c.c_ulonglong) for name in
                ("reads", "writes", "other", "read_bytes", "write_bytes", "other_bytes")]


class Limits(c.Structure):
    _fields_ = [("basic", Basic), ("io", Io), ("process_memory", c.c_size_t),
                ("job_memory", c.c_size_t), ("peak_process_memory", c.c_size_t),
                ("peak_job_memory", c.c_size_t)]


class CpuInfo(c.Structure):
    _fields_ = [("mask", c.c_size_t), ("relationship", w.DWORD), ("data", c.c_ulonglong * 2)]


class Resources:
    def __init__(self, cpus=2, memory_gib=4):
        if os.name != "nt":
            raise RuntimeError("This resource-bounded launcher currently supports Windows/Git Bash. "
                               "The Rust selfplay worker itself is portable.")
        self.k = k = c.WinDLL("kernel32", use_last_error=True)
        k.CreateJobObjectW.argtypes = [c.c_void_p, w.LPCWSTR]
        k.CreateJobObjectW.restype = w.HANDLE
        k.GetCurrentProcess.restype = w.HANDLE
        k.SetInformationJobObject.argtypes = [w.HANDLE, c.c_int, c.c_void_p, w.DWORD]
        k.AssignProcessToJobObject.argtypes = [w.HANDLE, w.HANDLE]
        k.QueryInformationJobObject.argtypes = [w.HANDLE, c.c_int, c.c_void_p, w.DWORD, c.c_void_p]
        k.GetProcessAffinityMask.argtypes = [w.HANDLE, c.POINTER(c.c_size_t), c.POINTER(c.c_size_t)]
        k.SetProcessAffinityMask.argtypes = [w.HANDLE, c.c_size_t]
        k.GetLogicalProcessorInformation.argtypes = [c.c_void_p, c.POINTER(w.DWORD)]
        current, system = c.c_size_t(), c.c_size_t()
        self.check(k.GetProcessAffinityMask(k.GetCurrentProcess(), c.byref(current), c.byref(system)))
        bits = [1 << n for n in range(c.sizeof(c.c_size_t) * 8) if current.value & (1 << n)]
        if not 1 <= cpus <= len(bits):
            raise ValueError(f"--cpus must be between 1 and {len(bits)} available logical CPUs")
        self.mask = sum(bits[:cpus])
        length = w.DWORD()
        k.GetLogicalProcessorInformation(None, c.byref(length))
        buffer = c.create_string_buffer(length.value)
        self.check(k.GetLogicalProcessorInformation(buffer, c.byref(length)))
        entries = (CpuInfo * (length.value // c.sizeof(CpuInfo))).from_buffer(buffer)
        self.cores = [e.mask & self.mask for e in entries if e.relationship == 0 and e.mask & self.mask]
        self.job = k.CreateJobObjectW(None, None)
        self.check(self.job)
        limits = Limits()
        # Subset affinity lets workers use individual cores inside the job's CPU cap.
        # Retain the total commit cap, below-normal priority and kill-on-owner-exit.
        limits.basic.flags = 0x200 | 0x10 | 0x4000 | 0x20 | 0x2000
        limits.basic.affinity = self.mask
        limits.basic.priority = 0x4000
        limits.job_memory = memory_gib * 1024**3
        self.check(k.SetInformationJobObject(self.job, 9, c.byref(limits), c.sizeof(limits)))
        self.check(k.AssignProcessToJobObject(self.job, k.GetCurrentProcess()))
        # Keep this handle open until process exit: closing a kill-on-close job would kill us too.

    @staticmethod
    def check(ok):
        if not ok:
            raise c.WinError(c.get_last_error())

    def pin(self, process, worker, workers):
        # When possible, one physical core (including its SMT sibling) per worker.
        if workers <= len(self.cores):
            expected = self.cores[worker]
            self.check(self.k.SetProcessAffinityMask(int(process._handle), expected))
            actual, system = c.c_size_t(), c.c_size_t()
            self.check(self.k.GetProcessAffinityMask(int(process._handle), c.byref(actual), c.byref(system)))
            if actual.value != expected:
                raise RuntimeError(f"Worker {worker} CPU affinity is {actual.value}, expected {expected}")

    def describe(self):
        limits = Limits()
        self.check(self.k.QueryInformationJobObject(self.job, 9, c.byref(limits), c.sizeof(limits), None))
        return {"logical_cpus": self.mask.bit_count(), "physical_cores": len(self.cores),
                "affinity_mask": self.mask, "memory_gib": limits.job_memory / 1024**3,
                "priority": "below-normal", "peak_commit_mib": limits.peak_job_memory / 1024**2}
