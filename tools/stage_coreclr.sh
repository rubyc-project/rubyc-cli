#!/bin/bash
# Stage all coreclr masters as transient .rc files mirroring upstream layout.
# Usage: stage_coreclr.sh <CoreLib-srcdir>
# Idempotent; post-build hook deletes the .rc files.
set -u
SRCROOT="$1"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Masters source: $RUBYC_CORECLR_MASTERS, else tools/coreclr-masters/
# (seeded from the upstream pin; re-seed it when the pin moves).
M="${RUBYC_CORECLR_MASTERS:-$SCRIPT_DIR/coreclr-masters}"
B="$SRCROOT/CoreCLR"
mkdir -p "$B/System" "$B/System/Runtime/CompilerServices" \
  "$B/System/Threading" "$B/System/Reflection" \
  "$B/System/Reflection/Emit" "$B/System/Runtime/InteropServices" \
  "$B/System/Diagnostics" "$B/System/Runtime/Loader" "$B/System/IO" \
  "$B/System/Runtime" "$B/System/Collections" "$B/System/Text" \
  "$B/Internal/Runtime/InteropServices" "$B/Internal/Runtime/CompilerHelpers" \
  "$B/Internal" "$B/Interop/Unix" "$B/System/Collections/Generic" \
  "$B/System/Runtime/ExceptionServices" "$B/System/Runtime/InteropServices/CustomMarshalers"
cp_if_exists() { [ -f "$2" ] && cp "$2" "$3"; }
for f in String.CoreCLR Enum.CoreCLR Array.CoreCLR RuntimeHandles Exception.CoreCLR \
    Buffer.CoreCLR RuntimeType.CoreCLR Type.CoreCLR Math.CoreCLR MathF.CoreCLR Delegate.CoreCLR \
    RuntimeType.GenericCache RuntimeType.BoxCache RuntimeType.CreateUninitializedCache AppContext.CoreCLR GC.CoreCLR Object.CoreCLR Environment.CoreCLR \
    TypedReference.CoreCLR BadImageFormatException.CoreCLR TypeLoadException.CoreCLR StartupHookProvider.CoreCLR ComAwareWeakReference.CoreCLR \
    ArgIterator CLRConfig Currency OleAutBinder RuntimeArgumentHandle RuntimeType.ActivatorCache ValueType Variant __Canon __ComObject; do
  cp_if_exists x "$M/$f.cs" "$B/System/$f.rc"
  cp_if_exists x "$M/$f" "$B/System/$f.rc"
done
cp_if_exists x "$M/CastHelpers.cs" "$B/System/Runtime/CompilerServices/CastHelpers.rc"
cp_if_exists x "$M/AsyncHelpers.CoreCLR.cs" "$B/System/Runtime/CompilerServices/AsyncHelpers.CoreCLR.rc"
cp_if_exists x "$M/AsyncProfiler.CoreCLR.cs" "$B/System/Runtime/CompilerServices/AsyncProfiler.CoreCLR.rc"
for f in GenericsHelpers InitHelpers RuntimeAsyncTaskContinuation StaticsHelpers VirtualDispatchHelpers AsyncHelpers.ValueTaskSourceContinuation; do
  cp_if_exists x "$M/CompilerServices/$f.cs" "$B/System/Runtime/CompilerServices/$f.rc"
done
for f in ManagedThreadId.CoreCLR ObjectHeader.CoreCLR Thread.CoreCLR Interlocked.CoreCLR SynchronizationContext.CoreCLR; do
  cp_if_exists x "$M/Threading/$f.cs" "$B/System/Threading/$f.rc"
done
for f in RuntimeMethodInfo.CoreCLR RuntimeConstructorInfo.CoreCLR MemberInfo.Internal \
    RuntimeAssembly RuntimeParameterInfo RuntimeCustomAttributeData MdImport Assembly.CoreCLR \
    MdConstant MethodBase.CoreCLR TypeNameResolver.CoreCLR AssemblyName.CoreCLR ModifiedType.CoreCLR MethodBaseInvoker.CoreCLR MethodInvoker.CoreCLR \
    ConstructorInfo.CoreCLR ConstructorInvoker.CoreCLR FieldInfo.CoreCLR MdFieldInfo RtFieldInfo RuntimeEventInfo RuntimeFieldInfo \
    RuntimeModule RuntimePropertyInfo RuntimeMethodBody RuntimeLocalVariableInfo RuntimeExceptionHandlingClause LoaderAllocator Associates; do
  cp_if_exists x "$M/Reflection/$f.cs" "$B/System/Reflection/$f.rc"
  cp_if_exists x "$M/Reflection/$f" "$B/System/Reflection/$f.rc"
done
cp_if_exists x "$M/Emit/DynamicMethod.CoreCLR.cs" "$B/System/Reflection/Emit/DynamicMethod.CoreCLR.rc"
cp_if_exists x "$M/Emit/SignatureHelper.cs" "$B/System/Reflection/Emit/SignatureHelper.rc"
cp_if_exists x "$M/Emit/RuntimeTypeBuilder.cs" "$B/System/Reflection/Emit/RuntimeTypeBuilder.rc"
mkdir -p "$B/System/Reflection/Metadata"
cp_if_exists x "$M/Metadata/MetadataUpdater.cs" "$B/System/Reflection/Metadata/MetadataUpdater.rc"
cp_if_exists x "$M/Emit/RuntimeAssemblyBuilder.cs" "$B/System/Reflection/Emit/RuntimeAssemblyBuilder.rc"
cp_if_exists x "$M/Emit/RuntimeModuleBuilder.cs" "$B/System/Reflection/Emit/RuntimeModuleBuilder.rc"
for f in RuntimeILGenerator RuntimeMethodBuilder RuntimeConstructorBuilder RuntimeFieldBuilder RuntimeEnumBuilder RuntimeEventBuilder \
    RuntimeLocalBuilder RuntimeParameterBuilder RuntimeGenericTypeParameterBuilder CustomAttributeBuilder DynamicILGenerator; do
  cp_if_exists x "$M/Emit/$f.cs" "$B/System/Reflection/Emit/$f.rc"
done
for f in NativeLibrary.CoreCLR Marshal.CoreCLR ComWrappers.CoreCLR GCHandle.CoreCLR TrackerObjectManager.CoreCLR; do
  cp_if_exists x "$M/InteropServices/$f.cs" "$B/System/Runtime/InteropServices/$f.rc"
done
for f in StackTrace.CoreCLR StackFrame.CoreCLR; do
  cp_if_exists x "$M/Diagnostics/$f.cs" "$B/System/Diagnostics/$f.rc"
done
cp_if_exists x "$M/Diagnostics/Debugger.cs" "$B/System/Diagnostics/Debugger.rc"
cp_if_exists x "$M/Diagnostics/EditAndContinueHelper.cs" "$B/System/Diagnostics/EditAndContinueHelper.rc"
cp_if_exists x "$M/Diagnostics/StackFrameHelper.cs" "$B/System/Diagnostics/StackFrameHelper.rc"
cp_if_exists x "$M/Diagnostics/ICustomDebuggerNotification.cs" "$B/System/Diagnostics/ICustomDebuggerNotification.rc"
cp_if_exists x "$M/Loader/AssemblyLoadContext.CoreCLR.cs" "$B/System/Runtime/Loader/AssemblyLoadContext.CoreCLR.rc"
cp_if_exists x "$M/IO/Stream.CoreCLR.cs" "$B/System/IO/Stream.CoreCLR.rc"
cp_if_exists x "$M/IO/FileLoadException.CoreCLR.cs" "$B/System/IO/FileLoadException.CoreCLR.rc"
cp_if_exists x "$M/IO/FileNotFoundException.CoreCLR.cs" "$B/System/IO/FileNotFoundException.CoreCLR.rc"
# Cross-library linked sources ($(LibrariesProjectRoot), Link-carried):
# System.Reflection.Metadata files CoreLib compiles (in-repo vendored
# sources, not a scratch dir).
mkdir -p "$B/Common/System/Reflection/Metadata"
LROOT="$SRCROOT/../../System.Reflection.Metadata/src/System/Reflection/Metadata"
for f in AssemblyNameInfo TypeName TypeNameParser TypeNameParserHelpers; do
  cp_if_exists x "$LROOT/$f.cs" "$B/Common/System/Reflection/Metadata/$f.rc"
done
# CommonPath sources from the CoreCLR csproj. The hook never reads that
# csproj (it scans src/libraries projects only), so copy these direct
# from the vendored checkout (e.g. VersionResilientHashCode, whose
# NestedTypeHashCode/ArrayTypeHashCode/etc. RuntimeType needs).
CROOT="$SRCROOT/../../Common/src"
mkdir -p "$B/Common/Internal" "$B/Common/System/Collections/Generic" "$B/Common/System"
cp_if_exists x "$CROOT/Internal/VersionResilientHashCode.cs" "$B/Common/Internal/VersionResilientHashCode.rc"
cp_if_exists x "$CROOT/System/Collections/Generic/ArrayBuilder.cs" "$B/Common/System/Collections/Generic/ArrayBuilder.rc"
cp_if_exists x "$CROOT/System/Experimentals.cs" "$B/Common/System/Experimentals.rc"
cp_if_exists x "$M/Runtime/RuntimeHelpers.CoreCLR.cs" "$B/System/Runtime/RuntimeHelpers.CoreCLR.rc"
cp_if_exists x "$M/Runtime/GCSettings.CoreCLR.cs" "$B/System/Runtime/GCSettings.CoreCLR.rc"
cp_if_exists x "$M/Runtime/DependentHandle.cs" "$B/System/Runtime/DependentHandle.rc"
cp_if_exists x "$M/Collections/EmptyReadOnlyDictionaryInternal.cs" "$B/System/Collections/EmptyReadOnlyDictionaryInternal.rc"
cp_if_exists x "$M/Text/StringBuilder.CoreCLR.cs" "$B/System/Text/StringBuilder.CoreCLR.rc"
cp_if_exists x "$M/Internal/Runtime/InteropServices/ComponentActivator.CoreCLR.cs" "$B/Internal/Runtime/InteropServices/ComponentActivator.CoreCLR.rc"
cp_if_exists x "$M/Internal/Runtime/CompilerHelpers/ThrowHelpers.cs" "$B/Internal/Runtime/CompilerHelpers/ThrowHelpers.rc"
cp_if_exists x "$M/Internal/Runtime/InteropServices/ComActivationContextInternal.cs" "$B/Internal/Runtime/InteropServices/ComActivationContextInternal.rc"
cp_if_exists x "$M/Internal/Runtime/InteropServices/ComActivator.PlatformNotSupported.cs" "$B/Internal/Runtime/InteropServices/ComActivator.PlatformNotSupported.rc"
cp_if_exists x "$M/Internal/Runtime/InteropServices/InMemoryAssemblyLoader.PlatformNotSupported.cs" "$B/Internal/Runtime/InteropServices/InMemoryAssemblyLoader.PlatformNotSupported.rc"
cp_if_exists x "$M/Internal/VersionResilientHashCode.CoreCLR.cs" "$B/Internal/VersionResilientHashCode.CoreCLR.rc"
cp_if_exists x "$M/Interop/Unix/Interop.Libraries.cs" "$B/Interop/Unix/Interop.Libraries.rc"
for f in ArraySortHelper.CoreCLR Comparer.CoreCLR ComparerHelpers EqualityComparer.CoreCLR; do
  cp_if_exists x "$M/Collections/Generic/$f.cs" "$B/System/Collections/Generic/$f.rc"
done
for f in RuntimePropertyBuilder SymbolMethod; do
  cp_if_exists x "$M/Emit/$f.cs" "$B/System/Reflection/Emit/$f.rc"
done
cp_if_exists x "$M/Reflection/Metadata/AssemblyExtensions.cs" "$B/System/Reflection/Metadata/AssemblyExtensions.rc"
cp_if_exists x "$M/Runtime/ControlledExecution.CoreCLR.cs" "$B/System/Runtime/ControlledExecution.CoreCLR.rc"
for f in AsmOffsets InternalCalls; do
  cp_if_exists x "$M/Runtime/ExceptionServices/$f.cs" "$B/System/Runtime/ExceptionServices/$f.rc"
done
for f in ComDataHelpers EnumeratorToEnumVariantMarshaler EnumeratorViewOfEnumVariant EnumVariantViewOfEnumerator ExpandoToDispatchExMarshaler TypeToTypeInfoMarshaler; do
  cp_if_exists x "$M/InteropServices/CustomMarshalers/$f.cs" "$B/System/Runtime/InteropServices/CustomMarshalers/$f.rc"
done
for f in DynamicInterfaceCastableHelpers IDispatchHelpers NativeLibrary.CoreCLR; do
  cp_if_exists x "$M/InteropServices/$f.cs" "$B/System/Runtime/InteropServices/$f.rc"
done
cp_if_exists x "$M/Reflection/InstanceCalliHelper.cs" "$B/System/Reflection/InstanceCalliHelper.rc"
cp_if_exists x "$M/StubHelpers.cs" "$B/System/StubHelpers.rc"
cp_if_exists x "$M/Runtime/JitInfo.CoreCLR.cs" "$B/System/Runtime/JitInfo.CoreCLR.rc"
cp_if_exists x "$M/Runtime/AsyncHelpers.RuntimeAsyncYielder.cs" "$B/System/Runtime/AsyncHelpers.RuntimeAsyncYielder.rc"
find "$B" -name "*.rc" | wc -l
