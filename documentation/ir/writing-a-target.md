# Writing a Target Backend

This guide walks through implementing a new target backend. We'll sketch
an ARM64 backend as the example.

## What You Need to Implement

1. **`CodeGenBackend::lower(&Program) -> MachineCode`** — the main entry
   point
2. **Register allocation** — map vregs to physical registers
3. **Instruction emission** — emit machine code for each IR instruction
4. **Relocation** — fix up addresses after code layout
5. **Image writing** — turn `MachineCode` into an ELF image (if targeting
   a native architecture)

## What the IR Gives You

- Three-address form (no in-place updates)
- Virtual registers (you do the register allocation)
- Basic blocks (structured control flow)
- Explicit instructions (no hidden semantics)
- A uniform 64-bit value model

## Step 1: Register Allocation

ARM64 has 31 general-purpose registers (x0-x30) + a stack pointer (sp).
The first 8 argument registers are x0-x7. The return value is in x0.

```rust
// Simplified: linear-scan allocation
// x0-x7: arguments / return
// x8-x15: callee-saved (preserved across calls)
// x16-x17: temporaries
// x18: platform register (avoid)
// x19-x28: callee-saved (general purpose)
// x29: frame pointer
// x30: link register (return address)
// sp: stack pointer

const CALLEE_SAVED: &[u8] = &[8, 9, 10, 11, 12, 13, 14, 15, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28];
const ARG_REGS: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7];
```

Use the same linear-scan algorithm as the x86_64 backend:

1. Compute liveness (backward dataflow over the CFG)
2. Build vreg intervals [first touch, last liveness]
3. Sweep left to right, assigning the first free register
4. Spill to stack if no register is free

## Step 2: Prologue and Epilogue

```asm
// Prologue
stp x29, x30, [sp, #-16]!   // save frame pointer and link register
mov x29, sp                  // set up frame pointer
sub sp, sp, #frame_size      // allocate stack frame
// ... save callee-saved registers that are used ...

// Epilogue
// ... restore callee-saved registers ...
mov sp, x29                  // restore stack pointer
ldp x29, x30, [sp], #16     // restore frame pointer and link register
ret                          // return
```

## Step 3: Instruction Emission

Map each IR instruction to ARM64 machine code:

```rust
fn emit_instr(&mut self, alloc: &Allocation, instr: &Instr) {
    match instr {
        Instr::Const { dst, imm } => {
            // movz xN, #imm  (or movk for upper bits)
            let r = alloc.loc[dst.0 as usize];
            self.emit_movz(r, *imm);
        }
        Instr::Bin { dst, op, a, b } => {
            let rd = alloc.loc[dst.0 as usize];
            let ra = alloc.loc[a.0 as usize];
            let rb = alloc.loc[b.0 as usize];
            match op {
                BinOp::Add => self.emit_add(rd, ra, rb),
                BinOp::Sub => self.emit_sub(rd, ra, rb),
                BinOp::Mul => self.emit_mul(rd, ra, rb),
                // ...
            }
        }
        Instr::CallFn { dst, fidx, args } => {
            // Set up arguments in x0-x7
            for (i, arg) in args.iter().enumerate() {
                let r = alloc.loc[arg.0 as usize];
                let arg_r = ARG_REGS[i.min(7)];
                if r != arg_r {
                    self.emit_mov(arg_r, r);
                }
            }
            // Call
            let target = self.fn_bases[*fidx];
            self.emit_bl(target);
            // Return value is in x0
            let dst_r = alloc.loc[dst.0 as usize];
            if dst_r != 0 {
                self.emit_mov(dst_r, 0); // x0
            }
        }
        // ...
    }
}
```

## Step 4: Calling Convention

ARM64 Linux (AArch64) ABI:

| Role | Register |
|---|---|
| Receiver (`this`) | x0 |
| Param 0 | x0 (if no receiver) or x1 (if receiver) |
| Param 1 | x1 (if no receiver) or x2 (if receiver) |
| Param 2-7 | x2-x7 (shifted by 1 if receiver) |
| Param 8+ | Stack |
| Return value | x0 |
| Return address | x30 (link register) |

## Step 5: Relocation

ARM64 uses different relocation types than x86_64:

- `AARCH64_MOVW_UW`: 16-bit immediate (movz)
- `AARCH64_MOVK_UW`: 16-bit immediate (movk, upper bits)
- `AARCH64_CALL`: 26-bit relative call (bl)
- `AARCH64_JUMP`: 26-bit relative jump (b)

```rust
fn relocate(&mut self, code_base: u64, data_base: u64, heap_base: u64) {
    for reloc in &self.relocs {
        match reloc.kind {
            RelocKind::Abs64 => {
                // Patch a 64-bit immediate (movz + movk sequence)
                let target = self.resolve(&reloc.sym, code_base, data_base, heap_base);
                self.patch_abs64(reloc.site, target);
            }
            RelocKind::Rel32Call => {
                // ARM64 calls are 26-bit relative (not 32-bit)
                let after_addr = code_base + reloc.site as u64 + 4;
                let rel = ((self.resolve(&reloc.sym, code_base, data_base, heap_base) as i64)
                    - after_addr as i64) / 4;
                // Patch 26-bit immediate
                self.patch_rel26(reloc.site, rel as u32);
            }
        }
    }
}
```

## Step 6: Image Writing

If targeting a native architecture, produce an ELF image:

```rust
impl ImageWriter for Arm64ImageWriter {
    fn write_image(&self, mc: &MachineCode, entry_preamble: bool) -> Result<Vec<u8>, TargetError> {
        // Build ELF header
        // Build program headers (PT_LOAD for code, data, heap)
        // Build section headers
        // Copy code and data into the image
        // Apply relocations
        Ok(image_bytes)
    }
}
```

## Tips

1. **Start with the x86_64 backend as a reference:** The x86_64 backend
   in `modules/targets/rubyc-target-linux-x86_64/` is the reference
   implementation. Study its structure before writing a new backend.

2. **Use `Raw` for instructions you can't express yet:** If you can't
   emit a particular instruction, emit a `udf` (undefined instruction)
   and continue. You can fill it in later.

3. **Test with simple programs first:** Start with `main() { return 0; }`
   and build up from there.

4. **Check the calling convention carefully:** A single wrong register
   assignment can corrupt the stack. Double-check your argument passing
   and return value handling.

5. **Use the existing `MachineCode` struct:** Don't invent a new output
   format. Use `MachineCode` with its `code`, `data`, `relocs`,
   `fn_bases`, etc. fields.
