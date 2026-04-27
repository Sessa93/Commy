# commy

`commy` is a work-in-progress Commodore 64 emulator written in Rust.

The current baseline focuses on the parts you need to grow a real emulator instead of starting from a monolithic prototype:

- a 6510 CPU core with a broader, tested instruction set for loops, stack use, indirect pointers, and interrupt return
- a C64 memory bus with ROM banking hooks plus minimal VIC-II, CIA, and SID devices
- a machine wrapper that can load `.prg` files into RAM
- a tiny CLI for stepping a program, mounting ROMs, and dumping CPU/video/device state

## Current Scope

Implemented now:

- `LDA`, `LDX`, `LDY` with immediate, zero-page, absolute, indirect, and selected indexed modes
- `STA`, `STX`, `STY` with zero-page, absolute, indirect, and selected indexed modes
- `TAX`, `TAY`, `TXA`, `TYA`, `TSX`, `TXS`
- `INX`, `INY`, `DEX`, `DEY`
- `INC`, `DEC`, `ADC`, `SBC`, `AND`, `ORA`, `EOR`, `BIT`, `CMP`, `CPX`, `CPY`
- `JMP` including indirect vector dispatch, `JSR`, `RTS`, `RTI`, `BEQ`, `BNE`, `BCC`, `BCS`, `BMI`, `BPL`, `BVC`, `BVS`
- `CLC`, `SEC`, `CLI`, `SEI`, `CLD`, `SED`, `CLV`, `PHA`, `PLA`, `PHP`, `PLP`, `NOP`, `BRK`
- reset vector handling
- BASIC, KERNAL, and character ROM slots
- C64 banking switches via the 6510 CPU port at `$0001`
- VIC-II raster stepping and a text-mode screen snapshot sourced from screen RAM
- VIC-II raster IRQ generation into the CPU with KERNAL-side acknowledge and return via `RTI`
- CIA timer countdown and SID voice phase stepping tied to the same bus tick path as the VIC-II
- ROM reset handlers that can execute directly from mapped KERNAL ROM bytes
- CIA timer IRQ delivery into the CPU with KERNAL-side acknowledge and return via `RTI`
- CIA2 timer-driven NMI delivery into the CPU, including edge-consumed NMI polling and KERNAL-side return via `RTI`

Not implemented yet:

- VIC-II timing and rendering
- SID audio
- CIA timers and keyboard/joystick matrix
- exact cycle timing, illegal opcodes, cartridges, tape, disk, and full KERNAL/BASIC boot flow

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

The CLI prints CPU state, the active reset vector, VIC raster position, CIA1 IRQ state, CIA2 NMI state, SID phase state, a small RAM window, and the first few lines of the text screen snapshot.

## Suggested Next Milestones

1. Expand opcode coverage and addressing modes until larger KERNAL and BASIC routines run without unsupported opcodes.
2. Flesh out the VIC-II beyond raster counters and text snapshots, then add real CIA keyboard/joystick behavior and SID waveform/envelope generation.
3. Refine the shared timing model from simple per-instruction ticking toward cycle-accurate coordination and more complete VIC/CIA interrupt behavior.
4. Boot through real ROMs far enough to reach a recognizable KERNAL/BASIC startup path instead of only smoke-tested reset, IRQ, and NMI handlers.
