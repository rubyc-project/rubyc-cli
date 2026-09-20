# Extension Points

How to extend the IR for a new language feature or target requirement.

## Rules for Extending the IR

1. **Add new `Instr` variants** — never change the meaning of an existing
   variant.
2. **Add tags in `serialize.rs`** — assign a new tag byte (next unused
   value).
3. **Update all backends** — every backend must handle the new instruction
   (or reject with `TargetError::Unsupported`).
4. **Bump `IR_FORMAT_VERSION`** — if the change is non-backwards-compatible.

## Adding a New Instruction

### Step 1: Add the Variant

```rust
// instructions.rs
pub enum Instr {
    // ... existing variants ...
    
    /// dst = sqrt(f64(a))
    FSqrt { dst: VReg, a: VReg },
}
```

### Step 2: Add Serialization

```rust
// serialize.rs
const tag::FSQRT: u8 = 0x3e;  // next unused tag

// In Writer::instr:
Instr::FSqrt { dst, a } => {
    self.byte(tag::FSQRT);
    self.vreg(*dst);
    self.vreg(*a);
}

// In Reader::instr:
tag::FSQRT => {
    let dst = self.vreg()?;
    let a = self.vreg()?;
    Ok(Instr::FSqrt { dst, a })
}
```

### Step 3: Update Backends

```rust
// x86_64 backend
Instr::FSqrt { dst, a } => {
    // sqrtsd xmm0, [addr]
    let src = alloc.loc[a.0 as usize];
    let dst_loc = alloc.loc[dst.0 as usize];
    self.emit_sqrtsd(&dst_loc, &src);
}

// ARM64 backend
Instr::FSqrt { dst, a } => {
    // fsqrt dN, dM
    let rd = alloc.loc[dst.0 as usize];
    let ra = alloc.loc[a.0 as usize];
    self.emit_fsqrt(rd, ra);
}
```

### Step 4: Bump Version (if needed)

If old backends can't handle the new instruction (because they don't
know the tag), bump the version:

```rust
pub const IR_FORMAT_VERSION: u32 = 3;  // was 2
```

Old backends will reject the new container with
`DecodeError::UnsupportedVersion`.

## Adding a New `BinOp` / `CmpOp` / `FBinOp` / `FCmpOp`

Same process as adding an instruction, but the new op is a variant of an
existing enum:

```rust
pub enum BinOp {
    // ... existing ops ...
    AddSat,  // saturating addition
}
```

Update `binop_byte` / `binop_from_byte` in `serialize.rs`, and update all
backends to handle the new op.

## Adding a New `ReceiverKind`

```rust
pub enum ReceiverKind {
    None,
    This,
    Explicit,
    Boxed,  // receiver is a Box<T> (Rust)
}
```

Update `receiver_byte` / `receiver_from_byte` in `serialize.rs`. Backends
that don't support the new kind should reject it.

## Adding a New `Sym` Variant

```rust
pub enum Sym {
    // ... existing variants ...
    TypeDescriptor(usize),  // for RTTI
}
```

Update `MachineCode::resolve` and the ELF writer to handle the new symbol.

## Adding a New `Terminator`

```rust
pub enum Terminator {
    // ... existing variants ...
    Switch { value: VReg, cases: Vec<(i64, BlockId)>, default: BlockId },
}
```

Update serialization and all backends.

## What NOT to Do

- **Don't change the meaning of an existing instruction.** If you need
  different semantics, add a new instruction.
- **Don't reuse tag bytes.** Tags are permanently assigned.
- **Don't skip updating backends.** Every backend must handle every
  instruction (or explicitly reject it).
- **Don't break the 64-bit value model.** All values are 64-bit words.
  If you need a wider value, pack it into two vregs.
