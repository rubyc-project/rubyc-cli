// Host stubs for RubyC standalone-executable smoke tests
// (`tests/exe_static_vm.rs`, `examples/exe_smoke/`).
//
// Managed library closures call a handful of CoreCLR runtime
// (`QCall`) natives that live in dotnet/runtime's native libraries,
// which RubyC does not build. Each stub below implements one such
// entry point with safe no-op semantics (documented per symbol), so
// smoke executables link fully statically. These are TEST-ONLY:
// production hosts must provide real implementations.
#define _GNU_SOURCE
#include <stdint.h>
#include <stdlib.h>
#include <unistd.h>
#include <malloc.h>
#include <dlfcn.h>
#include <sched.h>
#include <pthread.h>
#include <stdio.h>
#include <string.h>
#include <sys/utsname.h>

// Forward: tracked smoke allocator (defined with the GC stubs below).
static void *smoke_alloc(size_t n);

// StubHelpers.CreateLayoutClassMarshalStubs(...): generates marshaling
// stubs for layout classes. Null stubs (smoke never marshals layout
// classes through these entries).
void StubHelpers_CreateLayoutClassMarshalStubs(void *a, void *b) {
    (void)a;
    (void)b;
}

// SystemNative.GetCwd(buffer, len): current directory. Real: getcwd
// (null on failure, matching the PAL contract).
char *SystemNative_GetCwd(char *buffer, int32_t bufferLength) {
    if (!buffer || bufferLength <= 0) {
        return 0;
    }
    return getcwd(buffer, (size_t)bufferLength);
}

// SystemNative.GetHostName(name, len): host name. Real: gethostname
// (0 on success, matching the PAL contract).
int32_t SystemNative_GetHostName(char *name, int32_t nameLength) {
    if (!name || nameLength <= 0) {
        return -1;
    }
    return gethostname(name, (size_t)nameLength);
}

// SystemNative.GetUnixVersion(buffer, capacity): OS version string.
// Real: uname().release (0 on success).
int32_t SystemNative_GetUnixVersion(char *buffer, int32_t *capacity) {
    if (!buffer || !capacity || *capacity <= 0) {
        return -1;
    }
    struct utsname u;
    if (uname(&u) != 0) {
        return -1;
    }
    size_t n = strlen(u.release);
    if ((int32_t)n >= *capacity) {
        n = (size_t)(*capacity - 1);
    }
    memcpy(buffer, u.release, n);
    buffer[n] = 0;
    *capacity = (int32_t)n;
    return 0;
}

// AsyncHelpers.AddContinuationToExInternal(...): async continuation
// plumbing. No-op stub (smoke is synchronous; a hang here would
// prove otherwise — loud by design).
void AsyncHelpers_AddContinuationToExInternal(void) {}

// ThrowInvalidCastException: raises InvalidCast. Abort stub (reaching
// it proves a real failed cast — loud by design, like FailFast).
void ThrowInvalidCastException(void) {
    abort();
}

// IsInstanceOf_NoCacheLookup(...): type check without cache.
// Conservative false (forces slow paths elsewhere; a wrong false
// would surface as a failed cast — loud by design).
int32_t IsInstanceOf_NoCacheLookup(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// GenericHandleWorker(...): generic handle resolution worker. Null
// stub (smoke never resolves open generics at runtime).
void *GenericHandleWorker(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// InitClassHelper(...): runs a static constructor. No-op stub: the
// smoke image runs static initializers inline at startup (@init),
// so by the time managed code runs, classes are initialized. If a
// path truly needs lazy init, its absence surfaces as wrong behavior
// on the VM run — loud by design.
void InitClassHelper(void *a) {
    (void)a;
}

// ReflectionInvocation_RunClassConstructor/RunModuleConstructor:
// class/module constructor runners. No-ops (static initializers run
// inline; see above).
void ReflectionInvocation_RunClassConstructor(void *a) {
    (void)a;
}
void ReflectionInvocation_RunModuleConstructor(void *a) {
    (void)a;
}

// ReflectionInvocation.PrepareDelegate(...): prepares a delegate for
// invocation. No-op stub (smoke delegates, if any, need no prep).
void ReflectionInvocation_PrepareDelegate(void *a, void *b) {
    (void)a;
    (void)b;
}

// ObjectNative.AllocateUninitializedClone(obj): uninitialized clone
// (MemberwiseClone slow path). Allocates a zeroed 64-byte blob —
// enough for small objects; larger ones are a documented smoke
// limitation (a VM crash here proves real use).
void *ObjectNative_AllocateUninitializedClone(void *obj) {
    (void)obj;
    return calloc(1, 64);
}

// ReflectionInvocation.SizeOf(...): instance size query. Pointer
// size (correct for references; value types are a documented smoke
// limitation — same rationale as MarshalNative.SizeOfHelper).
int32_t ReflectionInvocation_SizeOf(void *a) {
    (void)a;
    return (int32_t)sizeof(void *);
}

// TypeHandle.CanCastTo_NoCacheLookup(...): cast check without cache.
// Conservative false (same rationale as IsInstanceOf above).
int32_t TypeHandle_CanCastTo_NoCacheLookup(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// TypeHandle.GetCorElementType(...): CorElementType of a type. Zero
// (ELEMENT_TYPE_END): smoke never switches on element types.
int32_t TypeHandle_GetCorElementType(void *a) {
    (void)a;
    return 0;
}

// Thread-static storage: a fixed table keyed by index (256 slots).
// Real implementation (static array; single-threaded smoke never
// contends). A second entry covers the by-method-table spelling,
// which aliases slot 0 (documented approximation).
#define SMOKE_THREAD_STATICS 256
static void *g_smoke_thread_statics[SMOKE_THREAD_STATICS] = {0};
void *GetThreadStaticsByIndex(int32_t index) {
    if (index < 0 || index >= SMOKE_THREAD_STATICS) {
        return 0;
    }
    return &g_smoke_thread_statics[index];
}
void *GetThreadStaticsByMethodTable(void *mt) {
    (void)mt;
    return &g_smoke_thread_statics[0];
}

// ResolveVirtualFunctionPointer(...): virtual dispatch resolver.
// Null stub (smoke never dispatches virtually through this entry —
// a VM crash proves otherwise, loud by design).
void *ResolveVirtualFunctionPointer(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// Interlocked.MemoryBarrierProcessWide(): full memory barrier. Real
// implementation.
void Interlocked_MemoryBarrierProcessWide(void) {
    __sync_synchronize();
}

// ThreadNative_* family: OS thread primitives. Real implementations
// via pthreads/sched where trivially correct; no-ops where smoke is
// single-threaded.
void ThreadNative_SpinWait(int32_t iterations) {
    for (int32_t i = 0; i < iterations; i++) {
#if defined(__x86_64__) || defined(__i386__)
        __asm__ volatile("pause");
#endif
    }
}
int32_t ThreadNative_YieldThread(void) {
    sched_yield();
    return 0;
}
void *ThreadNative_GetCurrentThread(void) {
    return (void *)pthread_self();
}
void ThreadNative_Initialize(void) {}
void ThreadNative_InformThreadNameChange(void *a, void *b) {
    (void)a;
    (void)b;
}
void ThreadNative_ResetAbort(void) {}

// DebugDebugger_* family: debugger hooks. Break aborts (reaching it
// proves a real Debug.Assert failure — loud by design); Launch is
// false (no debugger); Log writes to stderr (real); IsLogging is
// false.
void DebugDebugger_Break(void) {
    abort();
}
int32_t DebugDebugger_Launch(void) {
    return 0;
}
void DebugDebugger_Log(int32_t level, const char *message) {
    (void)level;
    if (message) {
        fputs(message, stderr);
    }
}
int32_t DebugDebugger_IsLoggingHelper(void) {
    return 0;
}

// StackFrame.GetMethodDescFromNativeIP / StackTrace.GetStackFramesInternal:
// stack walking needs unwind tables smoke hosts do not carry. Null
// stubs (smoke never walks stacks at runtime).
void *StackFrame_GetMethodDescFromNativeIP(void *ip) {
    (void)ip;
    return 0;
}
void *StackTrace_GetStackFramesInternal(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// AppContext.TryGetHostPropertyValue(name, retValue): reports a host
// property string. Unknown hosts have no properties: always false
// (0), leaving retValue untouched. Callers fall back to built-in
// defaults (e.g. well-known paths), which is correct on a bare host.
int32_t AppContext_TryGetHostPropertyValue(void *name, void *retValue) {
    (void)name;
    (void)retValue;
    return 0;
}

// AssemblyNative_* family: assembly metadata queries (names,
// versions, resources, types, load contexts). Null/false stubs:
// smoke assemblies are statically linked — there is no loader, no
// GAC, no resources, no LoadContext. A VM failure here proves a
// smoke path unexpectedly queries assembly metadata.
void *AssemblyNative_GetExecutingAssembly(void) {
    return 0;
}
void *AssemblyNative_GetEntryAssembly(void) {
    return 0;
}
void *AssemblyNative_GetCodeBase(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_GetTypeCore(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *AssemblyNative_GetTypeCoreIgnoreCase(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *AssemblyNative_GetExportedTypes(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_GetResource(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *AssemblyNative_GetModule(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_GetManifestResourceNames(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_GetReferencedAssemblies(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_GetManifestResourceInfo(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *AssemblyNative_GetVersion(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_GetLocale(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_GetSimpleName(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_GetHashAlgorithm(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_GetFlags(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_GetPublicKey(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_GetModules(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_GetForwardedType(void *a) {
    (void)a;
    return 0;
}
void *AssemblyNative_InternalTryGetRawMetadata(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *AssemblyNative_InitializeAssemblyLoadContext(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void AssemblyNative_PrepareForAssemblyLoadContextRelease(void *a) {
    (void)a;
}
void *AssemblyNative_LoadFromPath(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *AssemblyNative_GetLoadedAssemblies(void) {
    return 0;
}
void *AssemblyNative_GetLoadContextForAssembly(void *a) {
    (void)a;
    return 0;
}
void AssemblyNative_TraceAssemblyResolveHandlerInvoked(void *a, void *b, void *c, void *d) {
    (void)a;
    (void)b;
    (void)c;
    (void)d;
}

// CustomAttribute_* family: attribute blob parsing. Null stubs
// (smoke never reflects over attributes at runtime).
void *CustomAttribute_ParseAttributeUsageAttribute(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *CustomAttribute_CreateCustomAttributeInstance(void *a) {
    (void)a;
    return 0;
}
void *CustomAttribute_CreatePropertyOrFieldData(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// RuntimeModule_* family: module metadata. Null stubs (same
// statically-linked rationale as assemblies above).
void *RuntimeModule_GetFullyQualifiedName(void *a) {
    (void)a;
    return 0;
}
void *RuntimeModule_GetTypes(void *a) {
    (void)a;
    return 0;
}

// VersionResilientHashCode.TypeHashCode(...): stable type hash used
// only by interop type-map dictionaries. Zero stub: smoke never
// queries those maps (collisions would only matter there).
int32_t VersionResilientHashCode_TypeHashCode(void *a) {
    (void)a;
    return 0;
}

// FileLoadException.GetMessageForHR(...): message for an HRESULT
// load failure. Null stub (same rationale as above).
void *FileLoadException_GetMessageForHR(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// GetFileLoadExceptionMessage(...): message for assembly-load
// failures. Null stub (same rationale as the type-load twin above).
void *GetFileLoadExceptionMessage(void) {
    return 0;
}

// TypeMapLazyDictionary.ProcessAttributes(...): registers
// interop type maps via callbacks. No-op stub: smoke paths never
// query COM type maps (a wrong result here would surface as wrong
// interop behavior — loud by design, not silent).
void TypeMapLazyDictionary_ProcessAttributes(void) {}

// TrackerObjectManager.RegisterNativeObjectWrapperCache(...):
// registers the native-wrapper cache callbacks. No-op: no wrappers
// exist without tracking (see above).
void TrackerObjectManager_RegisterNativeObjectWrapperCache(void *a, void *b) {
    (void)a;
    (void)b;
}

// TrackerObjectManager.TryRegisterReferenceTrackerManager(...):
// COM reference-tracker registration. False stub: no tracking on a
// Linux smoke host (callers fall back to untracked lifetime).
int32_t TrackerObjectManager_TryRegisterReferenceTrackerManager(void *a) {
    (void)a;
    return 0;
}

// NativeLibrary.LoadByName(...): dlopen wrapper. Real
// implementation (returns the dlopen handle, null on failure —
// exactly the contract callers expect).
void *NativeLibrary_LoadByName(void *a, void *b, void *c, void *d) {
    (void)b;
    (void)c;
    (void)d;
    return dlopen((const char *)a, 2 /* RTLD_NOW */);
}

// MarshalNative.GetFunctionPointerForDelegateInternal(...):
// function pointer for a delegate. Null stub (same rationale).
void *MarshalNative_GetFunctionPointerForDelegateInternal(void *a) {
    (void)a;
    return 0;
}

// MarshalNative.GetDelegateForFunctionPointerInternal(...):
// delegate object for a raw function pointer. Null stub (needs a
// real delegate layout; smoke never marshals callbacks — loud on
// use by design).
void *MarshalNative_GetDelegateForFunctionPointerInternal(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// MarshalNative.GetExceptionForHR(...): exception object for an
// HRESULT failure. Null stub: smoke paths never fail COM calls (a
// null dereference here proves otherwise — loud by design).
void *MarshalNative_GetExceptionForHR(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// MarshalNative.HasLayout(...): whether a type has explicit layout.
// Conservative false (sequential layout assumed).
int32_t MarshalNative_HasLayout(void *a) {
    (void)a;
    return 0;
}

// MarshalNative.SizeOfHelper(...): unmanaged size of a type. Needs
// metadata layout tables smoke hosts do not carry: returns pointer
// size (correct for references; value types are a documented smoke
// limitation — a wrong size here would surface as corrupt interop,
// which is the desired loud signal, not silent success).
int32_t MarshalNative_SizeOfHelper(void *a, int32_t b) {
    (void)a;
    (void)b;
    return (int32_t)sizeof(void *);
}

// GCHandle alloc/free pair: tracks pinned/held object references.
// Identity implementation: the handle IS the target object pointer.
// This is exactly correct (not just a stub) for a non-moving,
// non-collecting runtime — weak handles never die, pinned objects
// never move, dereferencing a handle touches the real object. Free
// is a no-op (nothing is ever collected).
void *GCHandle_InternalAllocWithGCTransition(void **value, int32_t type) {
    (void)type;
    return value ? *value : 0;
}
void GCHandle_InternalFreeWithGCTransition(void *handle) {
    (void)handle;
}

// ComWrappers_* family: COM interop vtable plumbing. Null stubs:
// no COM on a Linux smoke host (a VM crash here proves real use).
void *ComWrappers_GetIUnknownImpl(void) {
    return 0;
}
void *ComWrappers_AllocateRefCountedHandle(void *a) {
    (void)a;
    return 0;
}
void *ComWrappers_GetIReferenceTrackerTargetVftbl(void *a) {
    (void)a;
    return 0;
}
void *ComWrappers_GetTaggedImpl(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ComWrappers_GetUntrackedAddRefRelease(void) {
    return 0;
}
void ComWrappers_RegisterIsRootedCallback(void *a) {
    (void)a;
}
void *ComWrappers_GetIReferenceTrackerTarget(void *a) {
    (void)a;
    return 0;
}

// ValueType.GetHashCodeStrategy(pMT, obj, offset, size, fieldMT):
// hashing strategy for a value type (None=0 means no fast path —
// always safe, only slower). Zeroes the out-params.
int32_t ValueType_GetHashCodeStrategy(void *pMT, void *obj, void *offset,
                                      void *size, void *fieldMT) {
    (void)pMT;
    (void)obj;
    if (offset) {
        *(unsigned int *)offset = 0;
    }
    if (size) {
        *(unsigned int *)size = 0;
    }
    if (fieldMT) {
        *(void **)fieldMT = 0;
    }
    return 0;
}

// MethodTable.CanCompareBitsOrUseFastGetHashCode(...): whether a
// type supports bitwise comparison or fast hashing. Conservative
// false (forces slow paths — always safe, only slower).
int32_t MethodTable_CanCompareBitsOrUseFastGetHashCode(void *a) {
    (void)a;
    return 0;
}

// GetTypeLoadExceptionMessage(...): message for type-load failures.
// Null stub: smoke paths never fail type loads (a null dereference
// here would prove otherwise — the desired loud signal).
void *GetTypeLoadExceptionMessage(void) {
    return 0;
}

// String.IsInterned(str): returns the interned reference or null.
// Consistent with the identity Intern above: returns the input
// (everything is trivially "interned" to itself).
void *String_IsInterned(void *str) {
    return str;
}

// String.Intern(str): returns the interned reference for a string.
// Identity stub (returns the input): a VM run printing correctly
// proves interning is never load-bearing in smoke paths. A real pool
// needs the managed string layout; see MdUtf8String above for the
// one layout-sensitive comparison implemented for real.
void *String_Intern(void *str) {
    return str;
}

// MdUtf8String.EqualsCaseInsensitive(lhs, rhs, len): metadata name
// comparison. Real implementation (ASCII case-fold; UTF-8 multibyte
// compares by value — documented smoke approximation).
int32_t MdUtf8String_EqualsCaseInsensitive(const char *lhs, const char *rhs,
                                           int cSz) {
    if (!lhs || !rhs) {
        return lhs == rhs;
    }
    for (int i = 0; i < cSz; i++) {
        unsigned char a = (unsigned char)lhs[i];
        unsigned char b = (unsigned char)rhs[i];
        if (a >= 'A' && a <= 'Z') {
            a += (unsigned char)('a' - 'A');
        }
        if (b >= 'A' && b <= 'Z') {
            b += (unsigned char)('a' - 'A');
        }
        if (a != b) {
            return 0;
        }
        if (a == 0) {
            return 1;
        }
    }
    return 1;
}

// Signature_* metadata family: signature blob queries. False/null
// stubs (smoke paths never inspect signatures at runtime).
void Signature_Init(void *a) {
    (void)a;
}
int32_t Signature_AreEqual(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *Signature_GetCustomModifiersAtOffset(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// ModuleHandle_* metadata family: module metadata queries (tokens,
// resolution, PE info). Null stubs: smoke paths resolve everything
// statically at compile time.
void *ModuleHandle_GetMDStreamVersion(void *a) {
    (void)a;
    return 0;
}
void *ModuleHandle_GetModuleType(void *a) {
    (void)a;
    return 0;
}
void *ModuleHandle_GetPEKind(void *a) {
    (void)a;
    return 0;
}
void *ModuleHandle_GetToken(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ModuleHandle_ResolveField(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ModuleHandle_ResolveMethod(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ModuleHandle_ResolveType(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// ModuleHandle.GetDynamicMethod(...): Reflection.Emit support.
// Null stub: smoke paths never emit dynamic methods.
void *ModuleHandle_GetDynamicMethod(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// RuntimeFieldHandle_* metadata family: field metadata queries.
// Null stubs (same rationale as the method family above).
void *RuntimeFieldHandle_GetEnCFieldAddr(void *a) {
    (void)a;
    return 0;
}
void *RuntimeFieldHandle_GetFieldDataReference(void *a) {
    (void)a;
    return 0;
}
void *RuntimeFieldHandle_GetRVAFieldInfo(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *RuntimeFieldHandle_GetValue(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *RuntimeFieldHandle_GetValueDirect(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void RuntimeFieldHandle_SetValue(void *a, void *b) {
    (void)a;
    (void)b;
}
void RuntimeFieldHandle_SetValueDirect(void *a, void *b) {
    (void)a;
    (void)b;
}

// RuntimeMethodHandle_* metadata family: method metadata queries
// needing runtime tables. False/null stubs (same rationale —
// reflection at runtime is out of smoke scope; GetFunctionPointer
// above covers the pointer case).
int32_t RuntimeMethodHandle_IsCAVisibleFromDecoratedType(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *RuntimeMethodHandle_ConstructInstantiation(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void RuntimeMethodHandle_Destroy(void *a) {
    (void)a;
}
void *RuntimeMethodHandle_GetMethodBody(void *a) {
    (void)a;
    return 0;
}
void *RuntimeMethodHandle_GetMethodInstantiation(void *a) {
    (void)a;
    return 0;
}
void *RuntimeMethodHandle_GetNativeCode(void *a) {
    (void)a;
    return 0;
}
void *RuntimeMethodHandle_GetStubIfNeededSlow(void *a) {
    (void)a;
    return 0;
}
void *RuntimeMethodHandle_GetTypicalMethodDefinition(void *a) {
    (void)a;
    return 0;
}
void *RuntimeMethodHandle_InvokeMethod(void *a, void *b, void *c) {
    (void)a;
    (void)b;
    (void)c;
    return 0;
}
void *RuntimeMethodHandle_StripMethodInstantiation(void *a) {
    (void)a;
    return 0;
}

// RuntimeMethodHandle.GetFunctionPointer(...): raw function pointer
// for a method. Null stub (a VM crash here proves real use).
void *RuntimeMethodHandle_GetFunctionPointer(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// QCall.FreeGCHandleForTypeHandle(...): releases the handle above.
// No-op (nothing was allocated).
void *QCall_FreeGCHandleForTypeHandle(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// QCall.GetGCHandleForTypeHandle(...): GC handle for a type. Null
// stub (same metadata rationale as above).
void *QCall_GetGCHandleForTypeHandle(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// RuntimeTypeHandle_* metadata family: reflection queries needing
// runtime metadata tables, which smoke hosts do not carry. Null/false
// stubs (a VM crash here proves real use — the desired loud signal).
// Signatures are arity-approximate (SysV caller-cleanup keeps that
// safe); only executed stubs need exact behavior, and smoke paths
// never execute these.
void *RuntimeTypeHandle_GetFields(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_GetAssemblySlow(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_GetConstraints(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_GetDeclaringMethodForGenericParameter(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_GetDeclaringTypeHandle(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_GetDeclaringTypeHandleForGenericVariable(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_GetGenericTypeDefinition(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_GetInstantiation(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_GetInterfaceMethodImplementation(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *RuntimeTypeHandle_GetInterfaces(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_GetModuleSlow(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_GetNumVirtualsAndStaticVirtuals(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_Instantiate(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *RuntimeTypeHandle_InternalAlloc(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_InternalAllocNoChecks(void *a) {
    (void)a;
    return 0;
}
int32_t RuntimeTypeHandle_IsEquivalentTo(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
int32_t RuntimeTypeHandle_IsVisible(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_MakeArray(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *RuntimeTypeHandle_MakeByRef(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_MakeFunctionPointer(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_MakePointer(void *a) {
    (void)a;
    return 0;
}
void *RuntimeTypeHandle_MakeSZArray(void *a) {
    (void)a;
    return 0;
}
void RuntimeTypeHandle_RegisterCollectibleTypeDependency(void *a, void *b) {
    (void)a;
    (void)b;
}
int32_t RuntimeTypeHandle_SatisfiesConstraints(void *a) {
    (void)a;
    return 0;
}
void RuntimeTypeHandle_VerifyInterfaceIsImplemented(void *a, void *b) {
    (void)a;
    (void)b;
}
void *RuntimeTypeHandle_AllocateComObject(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *RuntimeTypeHandle_AllocateTypeAssociatedMemory(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *RuntimeTypeHandle_AllocateTypeAssociatedMemoryAligned(void *a, void *b,
                                                            void *c) {
    (void)a;
    (void)b;
    (void)c;
    return 0;
}
void *RuntimeTypeHandle_ConstructName(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// RuntimeTypeHandle.GetArgumentTypesFromFunctionPointer(...):
// function-pointer signature metadata. Null stub (same rationale).
void *RuntimeTypeHandle_GetArgumentTypesFromFunctionPointer(void *a) {
    (void)a;
    return 0;
}

// RuntimeTypeHandle.GetMethodAt(...): method metadata lookup. Null
// stub (same rationale as the handle stubs above).
void *RuntimeTypeHandle_GetMethodAt(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// RuntimeTypeHandle.GetActivationInfo(...): activation metadata for
// interop. Null stub (same rationale as the handle stubs above).
void *RuntimeTypeHandle_GetActivationInfo(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}

// RuntimeTypeHandle.CreateInstanceForAnotherGenericParameter(...):
// instantiates a generic type for another parameter. Null stub:
// smoke paths never instantiate open generics at runtime.
void *RuntimeTypeHandle_CreateInstanceForAnotherGenericParameter(void *a,
                                                                void *b) {
    (void)a;
    (void)b;
    return 0;
}

// RuntimeTypeHandle.GetRuntimeTypeFromHandleSlow(handle): resolves a
// type from its handle via metadata tables, which smoke hosts do not
// carry. Null stub: statically-resolved paths never reach it.
void *RuntimeTypeHandle_GetRuntimeTypeFromHandleSlow(void *handle) {
    (void)handle;
    return 0;
}

// GCInterface_RefreshMemoryLimit(): re-reads the container memory
// limit. No-op: the smoke allocator tracks no limit.
void GCInterface_RefreshMemoryLimit(void) {}

// GCInterface_EnumerateConfigurationValues(dict, callback): reports
// GC configuration knobs through a callback. Reports none: the smoke
// allocator has no knobs. (The callback is never invoked.)
void GCInterface_EnumerateConfigurationValues(void *dict, void *callback) {
    (void)dict;
    (void)callback;
}

// GCInterface_GetGenerationBudget(generation): budget for a GC
// generation. Zero: no generations exist under a non-collecting
// allocator.
int64_t GCInterface_GetGenerationBudget(int32_t generation) {
    (void)generation;
    return 0;
}

// GCInterface_EnableNoGCRegionCallback(...): registers a callback
// for no-GC-region allocation pressure. No-op: regions are trivially
// satisfied and pressure callbacks never needed.
void GCInterface_EnableNoGCRegionCallback(void) {}

// GCInterface_WaitForFullGCApproach/Complete(timeout): waits for a
// full GC notification. No GC ever runs, so these always time out:
// return 0 (the timeout status), exactly as a quiet runtime would.
int32_t GCInterface_WaitForFullGCApproach(int32_t timeout) {
    (void)timeout;
    return 0;
}
int32_t GCInterface_WaitForFullGCComplete(int32_t timeout) {
    (void)timeout;
    return 0;
}

// GCInterface_GetTotalAllocatedBytesPrecise: cumulative allocated
// bytes. Tracked across the smoke allocator below (host calloc calls
// outside it are invisible — documented imprecision, fine for smoke).
static uint64_t g_smoke_allocated = 0;
static void *smoke_alloc(size_t n) {
    void *p = calloc(1, n);
    if (p) {
        g_smoke_allocated += (uint64_t)n;
    }
    return p;
}
int64_t GCInterface_GetTotalAllocatedBytesPrecise(void) {
    return (int64_t)g_smoke_allocated;
}

// GCInterface_ReRegisterForFinalize(o): re-queues an object for
// finalization. No-op: nothing is ever finalized (see above).
void GCInterface_ReRegisterForFinalize(void *o) {
    (void)o;
}

// GCInterface_WaitForPendingFinalizers: blocks until finalizers
// drain. No-op: the queue is always empty (see above).
void GCInterface_WaitForPendingFinalizers(void) {}

// GCInterface_GetNextFinalizableObject(target): walks the finalizer
// queue. Null stub: smoke hosts run no finalizers, so the queue is
// always empty.
void *GCInterface_GetNextFinalizableObject(void **target) {
    (void)target;
    return 0;
}

// GCInterface_AddMemoryPressure/RemoveMemoryPressure(bytes): GC
// accounting hints for unmanaged allocations. No-ops: nothing is
// ever collected, so pressure is moot.
void GCInterface_AddMemoryPressure(uint64_t bytes) {
    (void)bytes;
}
void GCInterface_RemoveMemoryPressure(uint64_t bytes) {
    (void)bytes;
}

// GCInterface_Collect(generation, mode, lowMemoryHelper): requests a
// collection. No-op: the smoke allocator never collects.
void GCInterface_Collect(int32_t generation, int32_t mode,
                         int32_t lowMemoryHelper) {
    (void)generation;
    (void)mode;
    (void)lowMemoryHelper;
}

// GCInterface_GetTotalMemory(forceFullCollection): bytes allocated.
// Real implementation: mallinfo bytes currently held (the smoke
// allocator never collects, so forceFullCollection is moot).
int64_t GCInterface_GetTotalMemory(int32_t forceFullCollection) {
    (void)forceFullCollection;
    return (int64_t)mallinfo2().uordblks;
}

// GCInterface_AllocateNewArray(typeHandle, length, flags, ret):
// GC array allocator. Same flat native layout as Array_CreateInstance
// (length u64 at 0, elements from 8, 8 bytes each — same smoke-only
// limitation).
void GCInterface_AllocateNewArray(void *typeHandle, int32_t length,
                                  int32_t flags, void **ret) {
    (void)typeHandle;
    (void)flags;
    if (!ret) {
        return;
    }
    *ret = 0;
    if (length < 0) {
        return;
    }
    uint64_t *arr = (uint64_t *)smoke_alloc(8 + (size_t)length * 8);
    if (!arr) {
        return;
    }
    arr[0] = (uint64_t)length;
    *ret = arr;
}

// GCInterface_StartNoGCRegion/EndNoGCRegion: no-GC regions. The
// smoke allocator (bump/calloc, never collects), so regions are
// trivially satisfied: return S_OK (0).
int32_t GCInterface_StartNoGCRegion(int64_t totalSize, int32_t lohSizeKnown,
                                    int64_t lohSize,
                                    int32_t disallowFullBlockingGC) {
    (void)totalSize;
    (void)lohSizeKnown;
    (void)lohSize;
    (void)disallowFullBlockingGC;
    return 0;
}
int32_t GCInterface_EndNoGCRegion(void) {
    return 0;
}

// ExceptionNative_* family: stack-trace capture and throw helpers
// needing runtime unwinding/metadata tables, which smoke hosts do
// not carry. Null stubs: smoke paths never throw (a VM crash here
// proves a real failure, which is the desired loud signal).
void *ExceptionNative_GetFrozenStackTrace(void *a) {
    (void)a;
    return 0;
}
void *ExceptionNative_GetMessageFromNativeResources(void *a) {
    (void)a;
    return 0;
}
void *ExceptionNative_GetMethodFromStackTrace(void *a) {
    (void)a;
    return 0;
}
void ExceptionNative_ThrowAmbiguousResolutionException(void) {
    abort();
}
void ExceptionNative_ThrowClassAccessException(void) {
    abort();
}
void ExceptionNative_ThrowEntryPointNotFoundException(void) {
    abort();
}
void ExceptionNative_ThrowFieldAccessException(void) {
    abort();
}
void ExceptionNative_ThrowMethodAccessException(void) {
    abort();
}

// Environment.GetProcessorCount(): number of available processors.
// Real implementation via sysconf.
int32_t Environment_GetProcessorCount(void) {
    long n = sysconf(_SC_NPROCESSORS_ONLN);
    return n > 0 ? (int32_t)n : 1;
}

// Environment.FailFast(...): terminates immediately on fatal error.
// Real implementation: abort. Smoke runs must never reach it (a VM
// abort proves a real failure, which is the desired loud signal).
void Environment_FailFast(void) {
    abort();
}

// Environment.Exit(exitCode): terminates the process. Real
// implementation (not a stub): forwards to the C library.
void Environment_Exit(int exitCode) {
    exit(exitCode);
}

// Delegate.GetMethodDesc(instance): method descriptor for delegate
// reflection. Null stub (see BindToMethodName rationale).
void *Delegate_GetMethodDesc(void **instance) {
    (void)instance;
    return 0;
}

// Delegate.AdjustTarget(targetMT, methodPtr): adjusts a delegate
// target for virtual dispatch. Null stub (same rationale).
void *Delegate_AdjustTarget(void *targetMT, void *methodPtr) {
    (void)targetMT;
    (void)methodPtr;
    return 0;
}

// Delegate.InitializeVirtualCallStub(d, methodPtr): installs the
// virtual-call stub. No-op stub (same rationale).
void Delegate_InitializeVirtualCallStub(void **d, void *methodPtr) {
    (void)d;
    (void)methodPtr;
}

// Delegate.CreateMethodInfo(methodDesc, retMethodInfo): materializes
// reflection metadata for a delegate target. Null stub: smoke paths
// never reflect over delegates at runtime.
void Delegate_CreateMethodInfo(void *methodDesc, void **retMethodInfo) {
    (void)methodDesc;
    if (retMethodInfo) {
        *retMethodInfo = 0;
    }
}

// Delegate.GetMulticastInvokeSlow(pMT): returns the slow-path invoke
// stub for a multicast delegate's method table. Null stub: smoke
// paths never invoke multicast delegates (a VM run completing proves
// it); linking requires the symbol to exist.
void *Delegate_GetMulticastInvokeSlow(void *pMT) {
    (void)pMT;
    return 0;
}

// Delegate.Construct(...): fills BindToMethodDetails for a delegate
// construction. No-op stub: smoke paths never construct delegates
// through this entry at runtime (a VM run printing correctly proves
// it); linking requires the symbol to exist.
void Delegate_Construct(void *a, void *b, void *c, void *d) {
    (void)a;
    (void)b;
    (void)c;
    (void)d;
}

// Delegate.BindToMethodInfo(...): same as BindToMethodName but
// binding by MethodInfo. Same false stub, same rationale.
int32_t Delegate_BindToMethodInfo(void *a, void *b, void *c, void *d,
                                  void *e, int32_t f, void *g, void *h) {
    (void)a;
    (void)b;
    (void)c;
    (void)d;
    (void)e;
    (void)f;
    (void)g;
    (void)h;
    return 0;
}

// Delegate.BindToMethodName(...): binds a delegate to a method by
// name using runtime metadata tables, which smoke hosts do not
// carry. Always false (bind failure); managed callers surface it as
// an ArgumentException. Safe to link: smoke paths never bind
// delegates at runtime (a VM run printing correctly proves it).
int32_t Delegate_BindToMethodName(void *a, void *b, void *c, void *d,
                                  void *e, int32_t f, void *g, void *h) {
    (void)a;
    (void)b;
    (void)c;
    (void)d;
    (void)e;
    (void)f;
    (void)g;
    (void)h;
    return 0;
}

// ClrConfig.GetConfigBoolValue(switchName, exist): reads a runtime
// configuration switch. Smoke hosts define none: false with
// exist=false (callers use compiled-in defaults).
int32_t ClrConfig_GetConfigBoolValue(void *switchName, int32_t *exist) {
    (void)switchName;
    if (exist) {
        *exist = 0;
    }
    return 0;
}

// Array.CreateInstance(type, rank, lengths, lowerBounds, fromArrayType,
// retArray): allocates a runtime array. Matches RubyC's native array
// layout exactly (length u64 at offset 0, elements from offset 8).
// Element size is not recoverable from the opaque type handle, so
// elements are 8 bytes (safe for all references and primitives up to
// 64 bits; larger value types are a documented smoke-only limitation).
// Only single-dimensional zero-based arrays are supported.
void Array_CreateInstance(void *type, int rank, int *pLengths,
                          int *pLowerBounds, int fromArrayType,
                          void **retArray) {
    (void)type;
    (void)pLowerBounds;
    (void)fromArrayType;
    if (!retArray) {
        return;
    }
    *retArray = 0;
    if (rank != 1 || !pLengths) {
        return;
    }
    int len = pLengths[0];
    if (len < 0) {
        return;
    }
    uint64_t *arr = (uint64_t *)smoke_alloc(8 + (size_t)len * 8);
    if (!arr) {
        return;
    }
    arr[0] = (uint64_t)len;
    *retArray = arr;
}

// Array.Ctor(pArrayMT, dwNumArgs, pArgList, retArray): multi-dimensional
// array allocator (CORINFO_HELP_NEW_MDARR). Lays out a flat buffer in
// RubyC's native array shape (length u64 at offset 0, elements from
// offset 8) with 8-byte elements; total elements = product of the
// dwNumArgs lengths. The MethodTable is opaque to the stub (element
// size unknown); same smoke-only 8-byte-element limitation as above.
void Array_Ctor(void *pArrayMT, unsigned int dwNumArgs, int *pArgList,
                void **retArray) {
    (void)pArrayMT;
    if (!retArray) {
        return;
    }
    *retArray = 0;
    if (!pArgList) {
        return;
    }
    size_t total = 1;
    for (unsigned int i = 0; i < dwNumArgs; i++) {
        int len = pArgList[i];
        if (len < 0) {
            return;
        }
        total *= (size_t)len;
    }
    uint64_t *arr = (uint64_t *)smoke_alloc(8 + total * 8);
    if (!arr) {
        return;
    }
    arr[0] = (uint64_t)total;
    *retArray = arr;
}

// AppContext.SetFirstChanceExceptionHandler: registers the runtime's
// first-chance exception hook. Smoke hosts run no such hook: no-op.
void AppContext_SetFirstChanceExceptionHandler(void) {}
// Bulk-generated cold-path stubs (each documented at first use above):
// reflection-emit scaffolding, diagnostics pipes, and loader queries
// that smoke paths never execute (a VM crash proves otherwise).
void *Array_GetElementConstructorEntrypoint(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
// DependentHandle alloc/free: (primary, dependent) pairs keeping
// `dependent` alive while `primary` lives. Heap-allocated pair
// structs (real dereferenceable memory — never small-int tokens,
// which crash if anything dereferences the handle). Nothing is ever
// collected, so Free just releases the pair block.
void *DependentHandle_InternalAllocWithGCTransition(void **primary,
                                                    void **dependent) {
    void **pair = (void **)calloc(1, 2 * sizeof(void *));
    if (!pair) {
        return 0;
    }
    pair[0] = primary ? *primary : 0;
    pair[1] = dependent ? *dependent : 0;
    return pair;
}
void *EventPipeInternal_CreateProvider(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *EventPipeInternal_GetNextEvent(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *EventPipeInternal_GetSessionInfo(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *EventPipeInternal_GetProvider(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *MetadataImport_GetMarshalAs(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *MethodBase_GetCurrentMethod(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ModuleBuilder_GetArrayMethodToken(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ModuleBuilder_GetMemberRef(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ModuleBuilder_GetMemberRefFromSignature(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ModuleBuilder_GetMemberRefOfFieldInfo(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ModuleBuilder_GetMemberRefOfMethodInfo(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ModuleBuilder_GetStringConstant(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ModuleBuilder_GetTokenFromTypeSpec(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ModuleBuilder_GetTypeRef(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *MultiCoreJIT_InternalSetProfileRoot(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *MultiCoreJIT_InternalStartProfile(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogContentionLockCreated(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogContentionStart(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogContentionStop(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogThreadPoolIODequeue(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogThreadPoolIOEnqueue(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogThreadPoolIOPack(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogThreadPoolMinMaxThreads(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogThreadPoolWorkerThreadAdjustmentAdjustment(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogThreadPoolWorkerThreadAdjustmentSample(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogThreadPoolWorkerThreadAdjustmentStats(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogThreadPoolWorkerThreadStart(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogThreadPoolWorkerThreadStop(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogThreadPoolWorkerThreadWait(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogThreadPoolWorkingThreadCount(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogWaitHandleWaitStart(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *NativeRuntimeEventSource_LogWaitHandleWaitStop(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ReflectionInvocation_GetBoxInfo(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *ReflectionSerialization_GetCreateUninitializedObjectInfo(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *StubHelpers_CreateCustomMarshaler(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *TypeBuilder_DefineEvent(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *TypeBuilder_DefineField(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *TypeBuilder_DefineMethod(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *TypeBuilder_DefineMethodSpec(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *TypeBuilder_DefineProperty(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *TypeBuilder_DefineType(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void *TypeBuilder_GetTokenFromSig(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
void DependentHandle_InternalFreeWithGCTransition(void *handle) {
    free(handle);
}
void EventPipeInternal_DefineEvent(void *a, void *b) {
    (void)a;
    (void)b;
}
void EventPipeInternal_DeleteProvider(void *a, void *b) {
    (void)a;
    (void)b;
}
void EventPipeInternal_Disable(void *a, void *b) {
    (void)a;
    (void)b;
}
void EventPipeInternal_EventActivityIdControl(void *a, void *b) {
    (void)a;
    (void)b;
}
void EventPipeInternal_WaitForSessionSignal(void *a, void *b) {
    (void)a;
    (void)b;
}
void EventPipeInternal_WriteEventData(void *a, void *b) {
    (void)a;
    (void)b;
}
void GlobalizationNative_EnumCalendarInfo(void *a, void *b) {
    (void)a;
    (void)b;
}
// GlobalizationNative.InitICUFunctions(...): loads ICU symbols. No-op:
// without ICU the runtime falls back to invariant globalization, which
// is correct for smoke (ASCII-only paths).
void GlobalizationNative_InitICUFunctions(void *a, void *b, void *c, void *d) {
    (void)a;
    (void)b;
    (void)c;
    (void)d;
}
void ModuleBuilder_SetFieldRVAContent(void *a, void *b) {
    (void)a;
    (void)b;
}
void StubHelpers_MulticastDebuggerTraceHelper(void *a, void *b) {
    (void)a;
    (void)b;
}
void StubHelpers_ValidateObject(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_AddInterfaceImpl(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_DefineCustomAttribute(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_DefineMethodImpl(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_DefineMethodSemantics(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_SetClassLayout(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_SetConstantValue(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_SetFieldLayoutOffset(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_SetMethodIL(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_SetMethodImpl(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_SetPInvokeData(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_SetParamInfo(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_SetParentType(void *a, void *b) {
    (void)a;
    (void)b;
}
void TypeBuilder_TermCreateClass(void *a, void *b) {
    (void)a;
    (void)b;
}
// SystemNative.GetEGid(): effective group id. Real implementation.
uint32_t SystemNative_GetEGid(void) {
    return (uint32_t)getegid();
}
// SystemNative.ConvertErrorPlatformToPal(err): errno mapping. Identity
// on Linux (platform == PAL codes).
int32_t SystemNative_ConvertErrorPlatformToPal(int32_t err) {
    return err;
}
// SystemNative.StrErrorR(err, buffer): message string. Real: writes via
// strerror_r into the provided buffer (assumed >= 256 bytes, the
// caller's contract).
void SystemNative_StrErrorR(int32_t err, char *buffer) {
    if (!buffer) {
        return;
    }
    strerror_r(err, buffer, 256);
}
// SystemNative.GetNonCryptographicallySecureRandomBytes /
// GetCryptographicallySecureRandomBytes: fill a buffer. Smoke-grade
// xorshift (documented non-crypto; never use smoke output as secrets).
void SystemNative_GetNonCryptographicallySecureRandomBytes(void *buf,
                                                           int32_t len) {
    if (!buf) {
        return;
    }
    unsigned char *p = (unsigned char *)buf;
    static uint64_t s = 0x9E3779B97F4A7C15ULL;
    for (int32_t i = 0; i < len; i++) {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        p[i] = (unsigned char)(s & 0xFF);
    }
}
void SystemNative_GetCryptographicallySecureRandomBytes(void *buf, int32_t len) {
    SystemNative_GetNonCryptographicallySecureRandomBytes(buf, len);
}
// SystemNative.LChflagsCanSetHiddenFlag / CanGetHiddenFlag: macOS
// chflags support. False on Linux (no such flags).
int32_t SystemNative_LChflagsCanSetHiddenFlag(void) {
    return 0;
}
int32_t SystemNative_CanGetHiddenFlag(void) {
    return 0;
}
// SystemNative.GetAllMountPoints(onFound, context): mount table
// enumeration via callback. Zero stub (never invokes the callback:
// smoke never enumerates mounts).
int32_t SystemNative_GetAllMountPoints(void *a, void *b) {
    (void)a;
    (void)b;
    return 0;
}
// SystemNative.ReadLink(path, buffer): symlink target. Real
// implementation (null-terminates on success, null stub contract
// otherwise).
void *SystemNative_ReadLink(const char *path, char *buffer) {
    if (!path || !buffer) {
        return 0;
    }
    ssize_t n = readlink(path, buffer, 4095);
    if (n < 0) {
        return 0;
    }
    buffer[n] = 0;
    return buffer;
}

// SystemNative_Poll(events, count, timeout_ms, triggered): poll(2)
// passthrough for console/key-availability paths. Reports no events
// (POLLNONE) when nothing is ready; writers never block on it.
#include <poll.h>
int32_t SystemNative_Poll(void *events, uint32_t count, int32_t timeout_ms, uint32_t *triggered) {
    (void)count;
    (void)timeout_ms;
    (void)events;
    if (triggered) {
        *triggered = 0;
    }
    return 0;
}

// Newly-referenced slow paths (emitted only when reachable; smoke
// paths reference but rarely execute them):
// - Stream_HasOverriddenSlow: no overrides in static images (false).
// - AssemblyHandle_GetManifestModuleSlow: leaves module null (dead
//   reflection-manifest path in smoke runs).
// - ObjectNative_GetHashCodeSlow: identity hash 0 (collisions only;
//   no live dictionaries in smoke runs).
// - TailCallHelp_AllocTailCallArgBufferInternal: tail calls unsupported
//   (null; smoke runs never tail-call).
// - ObjectHeader_GetOrCreateLockObject: leaves the lock slot null
//   (`lock` is a parse-time no-op single-threaded; direct Monitor
//   users must tolerate a null lock object as today).
int32_t Stream_HasOverriddenSlow(void *a, int32_t b) {
    (void)a;
    (void)b;
    return 0;
}
void AssemblyHandle_GetManifestModuleSlow(void *a, void *b) {
    (void)a;
    (void)b;
}
int32_t ObjectNative_GetHashCodeSlow(void *a) {
    (void)a;
    return 0;
}
void *TailCallHelp_AllocTailCallArgBufferInternal(int32_t size) {
    (void)size;
    return 0;
}
void ObjectHeader_GetOrCreateLockObject(void *a, void *b) {
    (void)a;
    (void)b;
}
// Debugger custom notification (no debugger attached in static
// exes): no-op. Reflection-emit generic param definition (no
// dynamic codegen): returns a zero token.
void DebugDebugger_CustomNotification(void *a) {
    (void)a;
}
int32_t TypeBuilder_DefineGenericParam(void *a, void *b, int32_t c, int32_t d, int32_t e, int32_t f, void *g) {
    (void)a;
    (void)b;
    (void)c;
    (void)d;
    (void)e;
    (void)f;
    (void)g;
    return 0;
}
// UnsafeAccessors generic-param resolution (no runtime type system):
// null handle. EventPipe session signal (no diagnostics server):
// false (no session signaled).
void *UnsafeAccessors_ResolveGenericParamToTypeHandle(void *a, int32_t b, uint32_t c) {
    (void)a;
    (void)b;
    (void)c;
    return 0;
}
int32_t EventPipeInternal_SignalSession(uint64_t a) {
    (void)a;
    return 0;
}
// Terminal/signal handling init (no terminal or signal server in
// static exes): claim success so PosixSignalRegistration statics
// initialize; actual Ctrl+C delivery is future work.
int32_t SystemNative_InitializeTerminalAndSignalHandling(void) {
    return 1;
}
// Native heap passthroughs (libc): NativeMemory, ArrayPool, and
// marshaling buffers need REAL memory (a zero stub would null-deref
// on first write). posix_memalign backs AlignedAlloc; plain
// malloc/calloc/realloc/free cover the rest. MMap/MUnmap stay
// zero-stubbed (unused in smoke paths; demand would fault loudly).
void *SystemNative_Malloc(uint64_t a) {
    return malloc((size_t)a);
}
void *SystemNative_Calloc(uint64_t a, uint64_t b) {
    return calloc((size_t)a, (size_t)b);
}
void *SystemNative_Realloc(void *a, uint64_t b) {
    return realloc(a, (size_t)b);
}
void SystemNative_Free(void *a) {
    free(a);
}
void *SystemNative_AlignedAlloc(uint64_t a, uint64_t b) {
    void *p = 0;
    if (posix_memalign(&p, (size_t)b, (size_t)a) != 0) {
        return 0;
    }
    return p;
}
void SystemNative_AlignedFree(void *a) {
    free(a);
}
void *SystemNative_AlignedRealloc(void *a, uint64_t b, uint64_t c) {
    void *p = 0;
    if (posix_memalign(&p, (size_t)b, (size_t)c) != 0) {
        return 0;
    }
    if (a != 0) {
        memcpy(p, a, (size_t)c);
        free(a);
    }
    return p;
}
// GC poll hook (no GC in static exes): no-op.
void ThreadNative_PollGC(void) {
}

// Raw-syscall time sources. Static executables are freestanding (no
// libc, no PLT): the linker's `--lib` body extractor copies function
// ranges verbatim, so any libc call (even `clock_gettime`) becomes a
// dangling PLT branch in the image. These use inline `syscall` only
// and are fully self-contained by construction.
#define SMOKE_NR_clock_gettime 228
#define SMOKE_NR_gettimeofday 96
#define SMOKE_NR_getcpu 309
#define SMOKE_CLOCK_MONOTONIC 1
struct smoke_timespec {
    int64_t tv_sec;
    int64_t tv_nsec;
};
struct smoke_timeval {
    int64_t tv_sec;
    int64_t tv_usec;
};
static long smoke_syscall3(long n, long a, long b, long c) {
    // NOTE: kept for documentation; the time sources below inline the
    // `syscall` via SMOKE_SYSCALL3 instead of calling this helper —
    // the `--lib` body extractor copies each symbol's range verbatim,
    // so an intra-library `call` would dangle in the static image.
    long ret;
    __asm__ volatile("syscall"
                     : "=a"(ret)
                     : "a"(n), "D"(a), "S"(b), "d"(c)
                     : "rcx", "r11", "memory");
    return ret;
}
// Inline raw syscall (self-contained: no call, no PLT — see above).
#define SMOKE_SYSCALL3(n, a, b, c, ret) \
    __asm__ volatile("syscall" \
                     : "=a"(ret) \
                     : "a"((long)(n)), "D"((long)(a)), "S"((long)(b)), "d"((long)(c)) \
                     : "rcx", "r11", "memory")

// SystemNative.GetTimestamp(): nanoseconds on CLOCK_MONOTONIC
// (matches Stopwatch.Frequency = 1e9). Real clock, not a stub: the
// CoreLib init path (`ProcessorIdCache.ProcessorNumberSpeedCheck`)
// spins until the timestamp advances, so a zero stub hangs the
// executable before Main.
int64_t SystemNative_GetTimestamp(void) {
    struct smoke_timespec ts;
    ts.tv_sec = 0;
    ts.tv_nsec = 0;
    long ret;
    SMOKE_SYSCALL3(SMOKE_NR_clock_gettime, SMOKE_CLOCK_MONOTONIC, (long)&ts, 0, ret);
    (void)ret;
    return ts.tv_sec * 1000000000 + ts.tv_nsec;
}

// SystemNative.GetLowResolutionTimestamp(): milliseconds since an
// arbitrary point (`Environment.TickCount64` contract). Real clock,
// same hang rationale as GetTimestamp above.
int64_t SystemNative_GetLowResolutionTimestamp(void) {
    struct smoke_timespec ts;
    ts.tv_sec = 0;
    ts.tv_nsec = 0;
    long ret;
    SMOKE_SYSCALL3(SMOKE_NR_clock_gettime, SMOKE_CLOCK_MONOTONIC, (long)&ts, 0, ret);
    (void)ret;
    return ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
}

// SystemNative.GetSystemTimeAsTicks(): UTC 100ns ticks since
// 0001-01-01 (DateTime tick contract: Unix epoch + 62135596800s).
int64_t SystemNative_GetSystemTimeAsTicks(void) {
    struct smoke_timeval tv;
    tv.tv_sec = 0;
    tv.tv_usec = 0;
    long ret;
    SMOKE_SYSCALL3(SMOKE_NR_gettimeofday, (long)&tv, 0, 0, ret);
    (void)ret;
    return (tv.tv_sec + 62135596800LL) * 10000000 + tv.tv_usec * 10;
}

// SystemNative.SchedGetCpu(): current CPU via SYS_getcpu (0 on
// failure; the managed -1 fallback path is not needed on Linux).
int32_t SystemNative_SchedGetCpu(void) {
    unsigned cpu = 0;
    long ret;
    SMOKE_SYSCALL3(SMOKE_NR_getcpu, (long)&cpu, 0, 0, ret);
    return ret != 0 ? 0 : (int32_t)cpu;
}

#define SMOKE_NR_write 1

// SystemNative.Write(fd, buffer, count): raw write syscall (real
// output — `Console.WriteLine` depends on it; a zero stub would
// swallow every byte). Self-contained inline `syscall`, no libc, no
// PLT (same freestanding rationale as the time sources above).
// Returns bytes written, or -1 on error (PAL contract).
//
// SafeHandle backstop: P/Invoke sites that bypass the compiler's
// `DangerousGetHandle` marshaling pass the SafeHandle OBJECT
// pointer as `fd` (heap address, never a small int). Detect and
// extract the `handle` field at its canonical slot (root-first
// layout: [vtable, refcount, handle, ...] → +16). Real fds stay
// small (< 0x100000; smoke exes open a handful); negatives pass
// through for the kernel to EBADF. Mirrors the compiler-side
// marshaling; either layer alone suffices.
int32_t SystemNative_Write(intptr_t fd, void *buffer, int32_t bufferSize) {
    long ret;
    if (!buffer || bufferSize <= 0) {
        return 0;
    }
    if (fd > 0x100000) {
        fd = *(intptr_t *)(fd + 16);
    }
    SMOKE_SYSCALL3(SMOKE_NR_write, (long)fd, (long)buffer, (long)bufferSize, ret);
    return (int32_t)ret;
}

#define SMOKE_NR_read 0
#define SMOKE_NR_close 3
#define SMOKE_NR_lseek 8
#define SMOKE_NR_unlink 87
#define SMOKE_NR_pread64 17
#define SMOKE_NR_pwrite64 18
#define SMOKE_NR_openat 257
#define SMOKE_AT_FDCWD (-100)

// File syscalls for the exe file-I/O smoke path (`FileStream` over
// real files). Same freestanding rationale as `Write` above (raw
// syscalls, no libc/PLT) and the same SafeHandle backstop: P/Invoke
// sites that bypass marshaling pass the SafeHandle OBJECT pointer as
// `fd` (heap address, never a small int); extract the `handle` field
// at its canonical slot (+16). Real fds stay small (< 0x100000);
// negatives pass through for the kernel to EBADF.
//
// LEAF-ONLY CONSTRAINT (copy-linker): exe builds extract each shim by
// symbol range, so shim-to-shim CALLS dangle — the callee's body is
// not copied and the call lands mid-instruction (SIGILL with args on
// the stack). Shared logic lives in macros (inlined into each body),
// never in helper functions. Keep every SystemNative_* body call-free.
#define SMOKE_UNWRAP_FD(f) do { \
    if ((f) > 0x100000) { (f) = *(intptr_t *)((f) + 16); } \
} while (0)

// CoreCLR PAL OpenFlags (`Interop.Sys.OpenFlags`) → Linux open(2)
// bits. The managed enum speaks PAL; the kernel speaks OS. Passing
// PAL raw opened the right path with the wrong disposition (PAL
// O_CREAT 0x20 is not OS O_CREAT 0x40 — new files died ENOENT).
// A macro (not a function) per the leaf-only constraint above.
#define SMOKE_MAP_OPEN_FLAGS(f) do { \
    int32_t _p = (f); int32_t _o = _p & 0x3; \
    if (_p & 0x0010) _o |= 0x80000; \
    if (_p & 0x0020) _o |= 0x40; \
    if (_p & 0x0040) _o |= 0x80; \
    if (_p & 0x0080) _o |= 0x200; \
    if (_p & 0x0100) _o |= 0x101000; \
    if (_p & 0x0200) _o |= 0x20000; \
    (f) = _o; \
} while (0)

// SystemNative.Open(path, flags, mode): openat(AT_FDCWD) is open(2).
// Returns the fd, or -1 on error (PAL contract).
int32_t SystemNative_Open(const char *path, int32_t flags, int32_t mode) {
    long ret;
    SMOKE_MAP_OPEN_FLAGS(flags);
    // Fourth syscall arg lives in r10 (not allocatable via "r").
    register long r10 __asm__("r10") = (long)mode;
    __asm__ volatile("syscall"
                     : "=a"(ret)
                     : "a"((long)SMOKE_NR_openat), "D"((long)SMOKE_AT_FDCWD),
                       "S"((long)path), "d"((long)flags), "r"(r10)
                     : "rcx", "r11", "memory");
    return (int32_t)ret;
}

// SystemNative.Read(fd, buffer, count): bytes read, 0 at EOF,
// or -1 on error (PAL contract).
int32_t SystemNative_Read(intptr_t fd, void *buffer, int32_t bufferSize) {
    long ret;
    if (!buffer || bufferSize <= 0) {
        return 0;
    }
    SMOKE_UNWRAP_FD(fd);
    SMOKE_SYSCALL3(SMOKE_NR_read, (long)fd, (long)buffer, (long)bufferSize, ret);
    return (int32_t)ret;
}

// SystemNative.Close(fd): 0 on success, -1 on error (PAL contract).
int32_t SystemNative_Close(intptr_t fd) {
    long ret;
    long zero = 0;
    SMOKE_UNWRAP_FD(fd);
    __asm__ volatile("syscall"
                     : "=a"(ret)
                     : "a"((long)SMOKE_NR_close), "D"((long)fd), "S"(zero), "d"(zero)
                     : "rcx", "r11", "memory");
    return (int32_t)ret;
}

// SystemNative.LSeek(fd, offset, whence): new position, or -1 on
// error (PAL contract).
int64_t SystemNative_LSeek(intptr_t fd, int64_t offset, int32_t whence) {
    long ret;
    SMOKE_UNWRAP_FD(fd);
    __asm__ volatile("syscall"
                     : "=a"(ret)
                     : "a"((long)SMOKE_NR_lseek), "D"((long)fd), "S"((long)offset),
                       "d"((long)whence)
                     : "rcx", "r11", "memory");
    return (int64_t)ret;
}

// SystemNative.Unlink(path): remove a probe artifact (cleanup only;
// 0 on success, -1 on error, PAL contract).
int32_t SystemNative_Unlink(const char *path) {
    long ret;
    long zero = 0;
    if (!path) {
        return -1;
    }
    __asm__ volatile("syscall"
                     : "=a"(ret)
                     : "a"((long)SMOKE_NR_unlink), "D"((long)path), "S"(zero), "d"(zero)
                     : "rcx", "r11", "memory");
    return (int32_t)ret;
}

// Positional I/O for `RandomAccess` (`File.WriteAllBytes` writes via
// `PWrite`; reads via `PRead`). A zero stub here would return 0
// (no progress), sending `WriteAtOffset`'s retry loop around forever
// — these must be real. Same SafeHandle backstop as above.
int32_t SystemNative_PRead(intptr_t fd, void *buffer, int32_t bufferSize, int64_t fileOffset) {
    long ret;
    if (!buffer || bufferSize <= 0) {
        return 0;
    }
    SMOKE_UNWRAP_FD(fd);
    register long r10 __asm__("r10") = (long)fileOffset;
    __asm__ volatile("syscall"
                     : "=a"(ret)
                     : "a"((long)SMOKE_NR_pread64), "D"((long)fd), "S"((long)buffer),
                       "d"((long)bufferSize), "r"(r10)
                     : "rcx", "r11", "memory");
    return (int32_t)ret;
}

int32_t SystemNative_PWrite(intptr_t fd, void *buffer, int32_t bufferSize, int64_t fileOffset) {
    long ret;
    if (!buffer || bufferSize <= 0) {
        return 0;
    }
    SMOKE_UNWRAP_FD(fd);
    register long r10 __asm__("r10") = (long)fileOffset;
    __asm__ volatile("syscall"
                     : "=a"(ret)
                     : "a"((long)SMOKE_NR_pwrite64), "D"((long)fd), "S"((long)buffer),
                       "d"((long)bufferSize), "r"(r10)
                     : "rcx", "r11", "memory");
    return (int32_t)ret;
}
