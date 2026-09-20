# Bare-metal targets

RubyC can compile to freestanding x86-64 binaries with no OS underneath.

## Status

The `baremetal-x86_64` target is planned. When complete it will produce
flat binary output suitable for QEMU (`-kernel` flag) or a bootloader.

## Testing

```bash
qemu-system-x86_64 -kernel output.bin -serial stdio
```

Serial I/O via port `0xE9` works with QEMU's `-debugcon stdio` option.
