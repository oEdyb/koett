import ctypes,time,pathlib,os
class Usage(ctypes.Structure):
 _fields_=[('uuid',ctypes.c_ubyte*16)]+[(n,ctypes.c_uint64) for n in ['user','system','idleWakeups','interruptWakeups','pageins','wired','resident','footprint','started','exited']]
lib=ctypes.CDLL('/usr/lib/libproc.dylib');lib.proc_pid_rusage.argtypes=[ctypes.c_int,ctypes.c_int,ctypes.c_void_p]
def usage(pid):
 value=Usage();assert lib.proc_pid_rusage(pid,0,ctypes.byref(value))==0
 return {'monotonic':time.monotonic(),'userNanoseconds':value.user,'systemNanoseconds':value.system,'residentBytes':value.resident,'physicalFootprintBytes':value.footprint}
def delta(before,after):
 return {'intervalSeconds':after['monotonic']-before['monotonic'],'processCPUPercentOneCore':100*((after['userNanoseconds']+after['systemNanoseconds'])-(before['userNanoseconds']+before['systemNanoseconds']))/1e9/(after['monotonic']-before['monotonic']),'before':before,'after':after}
def logical_bytes(root):
 return sum(p.stat().st_size for p in pathlib.Path(root).rglob('*') if p.is_file())
