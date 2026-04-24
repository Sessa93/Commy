# commy

`commy` is a work-in-progress Commodore 64 emulator written in Rust.

The current baseline focuses on the parts you need to grow a real emulator instead of starting from a monolithic prototype:

- a 6510 CPU core with a broader, tested instruction set for loops, stack use, and indexed memory access
- a C64 memory bus with ROM banking hooks and a minimal VIC-II device
- a machine wrapper that can load `.prg` files into RAM
- a tiny CLI for stepping a program, mounting ROMs, and dumping CPU/video state

## Current Scope

Implemented now:

- `LDA`, `LDX`, `LDY` with immediate, zero-page, absolute, and selected indexed modes
- `STA`, `STX`, `STY` with zero-page, absolute, and selected indexed modes
- `TAX`, `TAY`, `TXA`, `TYA`, `TSX`, `TXS`
- `INX`, `INY`, `DEX`, `DEY`
- `INC`, `DEC`, `ADC`, `SBC`, `AND`, `ORA`, `EOR`, `CMP`, `CPX`, `CPY`
- `JMP`, `JSR`, `RTS`, `BEQ`, `BNE`, `BCC`, `BCS`, `BMI`, `BPL`, `BVC`, `BVS`
- `CLC`, `SEC`, `CLI`, `SEI`, `CLD`, `SED`, `CLV`, `PHA`, `PLA`, `PHP`, `PLP`, `NOP`, `BRK`
- reset vector handling
- BASIC, KERNAL, and character ROM slots
- C64 banking switches via the 6510 CPU port at `$0001`
- VIC-II raster stepping and a text-mode screen snapshot sourced from screen RAM

Not implemented yet:

- VIC-II timing and rendering
- SID audio
- CIA timers and keyboard/joystick matrix
- interrupts, exact cycle timing, illegal opcodes, cartridges, tape, disk, and KERNAL boot flow

## Usage

Run the test suite:

```bash
cargo test
```

Run the built-in demo program:

```bash
cargo run -- --steps 16
```

Run a Commodore 64 PRG file:

```bash
cargo run -- path/to/program.prg --steps 5000
```

Run with ROM images mounted:

```bash
cargo run -- --kernal-rom roms/kernal.rom --basic-rom roms/basic.rom --char-rom roms/chargen.rom --steps 20000
```

The CLI prints CPU state, the active reset vector, VIC raster position, a small RAM window, and the first few lines of the text screen snapshot.

## Suggested Next Milestones

1. Expand opcode coverage until you can run more KERNAL-adjacent machine code.
2. Flesh out the VIC-II beyond raster counters and text snapshots, then replace the remaining SID and CIA placeholders.
3. Add a real shared timing model across CPU, VIC-II, CIA, and SID instead of only ticking the VIC from CPU cycles.
4. Boot through real ROMs far enough to enter KERNAL/BASIC startup instead of only exposing the mapped reset vector.
