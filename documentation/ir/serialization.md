# Serialization Format

The IR is serialized to a versioned byte container. This is the plugin
ABI boundary: target plugins receive the IR as bytes and deserialize it
into a `Program` to lower to machine code.

## Container Header

Every IR container starts with a header:

```
Offset  Size  Field
0       4     Magic: b"IR\x00"
4       4     Kind: b"IR\x00" (same as magic for IR)
8       4     Version: u32 LE (currently 2)
```

The magic and kind are both `"IR\x00"` (4 bytes each). The version
number is incremented whenever the format changes in a
non-backwards-compatible way.

## Tag Bytes

Each element in the serialized stream is prefixed with a 1-byte tag.
Tags are assigned in order of appearance in the `Program` struct.

### Program Tags

| Tag | Value | Meaning |
|---|---|---|
| 0x01 | PROGRAM | Start of the program body |
| 0x02 | FUNCTION | A function within the program |
| 0x03 | VTABLE | A vtable entry |
| 0x04 | IMPORT | An import entry |
| 0x05 | EXPORT | An export entry |
| 0x06 | STRINGS | The string blob |
| 0x07 | NAMES | The names table |
| 0x08 | ENTRY | The entry point |
| 0x09 | HEAP | The heap size |

### Function Tags

| Tag | Value | Meaning |
|---|---|---|
| 0x10 | FUNC_HEADER | Function name, receiver, arity, vreg count |
| 0x11 | BLOCK | A basic block |
| 0x12 | INSTR | An instruction |
| 0x13 | TERM | A terminator |

### Instruction Tags

| Tag | Value | Meaning |
|---|---|---|
| 0x20 | CONST | `Const { dst, imm }` |
| 0x21 | FCONST | `FConst { dst, bits }` |
| 0x22 | MOV | `Mov { dst, a }` |
| 0x23 | RAW | `Raw { bytes }` |
| 0x24 | BIN | `Bin { dst, op, a, b }` |
| 0x25 | NEG | `Neg { dst, a }` |
| 0x26 | CALL_FN | `CallFn { dst, fidx, args }` |
| 0x27 | CALL_VIRT | `CallVirt { dst, slot, recv, args }` |
| 0x28 | CALL_BY_NAME | `CallByName { dst, recv, name, args }` |
| 0x29 | CALL_IMPORT | `CallImport { dst, import, args }` |
| 0x2a | ALLOC | `Alloc { dst, bytes }` |
| 0x2b | STACK_ALLOC | `StackAlloc { dst, bytes }` |
| 0x2c | VTABLE_ADDR | `VTableAddr { dst, class }` |
| 0x2d | LOAD_FIELD | `LoadField { dst, base, off }` |
| 0x2e | STORE_FIELD | `StoreField { base, off, src }` |
| 0x2f | WRITE_STR | `WriteStr { off, len }` |
| 0x30 | EWRITE_STR | `EWriteStr { off, len }` |
| 0x31 | WRITE_INT | `WriteInt { src }` |
| 0x32 | EWRITE_INT | `EWriteInt { src }` |
| 0x33 | SCAN_INT | `ScanInt { dst }` |
| 0x34 | RETAIN | `Retain { src }` |
| 0x35 | RELEASE | `Release { src, bytes }` |
| 0x36 | DROP | `Drop { src }` |
| 0x37 | UNWIND | `Unwind { src }` |
| 0x38 | RESUME | `Resume { dst, args }` |
| 0x39 | FBin | `FBin { dst, op, a, b }` |
| 0x3a | FCmp | `FCmp { dst, op, a, b }` |
| 0x3b | FNeg | `FNeg { dst, a }` |
| 0x3c | CMP | `Cmp { dst, op, a, b }` |
| 0x3d | EXIT | `Exit` |

### Terminator Tags

| Tag | Value | Meaning |
|---|---|---|
| 0x40 | RET | `Ret { src }` |
| 0x41 | BRANCH | `Branch { cond, if_true, if_false }` |

## Encoding Rules

- **Integers:** All multi-byte integers are little-endian.
- **u32:** 4 bytes LE
- **u64:** 8 bytes LE
- **i64:** 8 bytes LE (two's complement)
- **u32 (vreg):** 4 bytes LE
- **Strings:** u32 LE length prefix + UTF-8 bytes
- **Vec<T>:** u32 LE count + each element
- **Option<T>:** 1 byte (0 = None, 1 = Some) + T if Some
- **Bool:** 1 byte (0 or 1)

## Example: Serialized Program

```
// Container header
IR\x00 IR\x00 02 00 00 00

// PROGRAM tag
01

// 1 function
01 00 00 00

// Function 0
10
  // name: "main" (4 bytes)
  04 00 00 00 6D 61 69 6E
  // receiver: None (0)
  00
  // arity: 0
  00 00 00 00
  // vregs: 1
  01 00 00 00
  // 1 block
  01 00 00 00
  // Block 0
  11
    // 1 instruction
    01 00 00 00
    // Const { dst: 0, imm: 0 }
    20
    00 00 00 00          // dst = 0
    00 00 00 00 00 00 00 00  // imm = 0
    // Terminator: Ret(None)
    13
    40
    00                   // None
// 0 vtables
00 00 00 00
// 0 imports
00 00 00 00
// 0 exports
00 00 00 00
// strings: empty
06
00 00 00 00
// names: empty
07
00 00 00 00
// entry: None
08
00
// heap_size: 0
09
00 00 00 00 00 00 00 00
```

## Adding a New Instruction

1. Add the variant to the `Instr` enum in `instructions.rs`.
2. Assign a new tag byte in `serialize.rs` (next unused value).
3. Add encoding in `Writer::instr`.
4. Add decoding in `Reader::instr`.
5. Update all backends to handle the new instruction (or reject with
   `TargetError::Unsupported`).
6. Bump `IR_FORMAT_VERSION` if the change is non-backwards-compatible.

## Versioning Strategy

- **Backwards-compatible changes** (adding a new instruction that old
  backends can ignore): no version bump. Old backends reject the unknown
  tag with `DecodeError::UnknownNodeTag`.
- **Non-backwards-compatible changes** (changing the meaning of an
  existing tag, removing a tag): bump the version. Old containers with
  the old version are rejected.
