// rvmodel_macros.h - abundance-rv32i-max (ACT4 self-checking framework)
//
// Halt mechanism: HTIF tohost write (Sail's exit protocol).
//   Writing (exit_code << 1) | 1 to tohost signals exit.
//   Code 0 = pass: write 1. Code != 0 = fail: write (code<<1)|1.
//
// No console, no timer, no interrupts implemented.

// ---------------------------------------------------------------------------
// HTIF tohost helper: writes VALUE to tohost then spins.
// Clobbers t0, t1.
// ---------------------------------------------------------------------------
#define _RVMODEL_HTIF_EXIT(value)       \
    .option push;                       \
    .option norvc;                      \
    la   t0, tohost;                    \
    li   t1, value;                     \
    sw   t1, 0(t0);                     \
    1: j 1b;                            \
    .option pop

// ---------------------------------------------------------------------------
// Required: halt with pass (exit code 0 -> tohost = 1)
// ---------------------------------------------------------------------------
#define RVMODEL_HALT_PASS  _RVMODEL_HTIF_EXIT(1)

// ---------------------------------------------------------------------------
// Required: halt with fail (exit code 1 -> tohost = 3, i.e. (1<<1)|1)
// ---------------------------------------------------------------------------
#define RVMODEL_HALT_FAIL  _RVMODEL_HTIF_EXIT(3)

// ---------------------------------------------------------------------------
// Legacy RVMODEL_HALT (used by signature-phase ELFs): pass exit.
// ---------------------------------------------------------------------------
#define RVMODEL_HALT  RVMODEL_HALT_PASS

// ---------------------------------------------------------------------------
// Boot macro - runs inside `rvmodel_boot` before the jump to `rvtest_init`.
//
// Emits a minimal direct-mode trap handler inline, branching over it, then
// writes its address into mtvec (direct mode: low 2 bits = 0).
//
// Handler contract: skip the faulting instruction and return. The faulting
// instruction may be a 16-bit compressed encoding (Zca is enabled), so the
// handler must not blindly add 4 - it reads the low halfword at mepc and
// advances by 4 only if bits[1:0] == 0b11 (a 32-bit instruction), otherwise
// by 2. This mirrors the framework's own default trap handler in
// rvtest_trap_handler.h. mepc is always at least 2-byte aligned, so the lhu
// cannot itself misalign-fault. No register saving needed - the test
// framework doesn't inspect register state across a trap.
// ---------------------------------------------------------------------------
#define RVMODEL_BOOT                            \
    .option push;                               \
    .option norvc;                              \
    .option arch, +zicsr;                       \
    j       1f;                                 \
    .align 2;                                   \
    .global rvmodel_trap_handler;               \
    rvmodel_trap_handler:                       \
        csrr    t0, mepc;                       \
        lhu     t1, 0(t0);                      \
        andi    t1, t1, 3;                      \
        addi    t1, t1, 1;                      \
        srli    t1, t1, 2;                      \
        addi    t1, t1, 1;                      \
        slli    t1, t1, 1;                      \
        add     t0, t0, t1;                     \
        csrw    mepc, t0;                       \
        mret;                                   \
    .align 2;                                   \
    1:                                          \
    la      t0, rvmodel_trap_handler;           \
    csrw    mtvec, t0;                          \
    .option pop

// ---------------------------------------------------------------------------
// Data section placement: default .data section is fine.
// ---------------------------------------------------------------------------
#define RVMODEL_DATA_SECTION \
    .pushsection .tohost,"aw",@progbits;                \
    .align 8; .global tohost; tohost: .dword 0;         \
    .align 8; .global fromhost; fromhost: .dword 0;     \
    .popsection

// ---------------------------------------------------------------------------
// Signature region markers (16-byte aligned per ACT4 spec).
// ---------------------------------------------------------------------------
#define RVMODEL_DATA_BEGIN              \
    .align 4;                           \
    .global begin_signature;            \
    begin_signature:

#define RVMODEL_DATA_END                \
    .align 4;                           \
    .global end_signature;              \
    end_signature:

// ---------------------------------------------------------------------------
// Console I/O: no console available, leave blank.
// The macros take register arguments (_R1, _R2, _R3) and a string pointer.
// ---------------------------------------------------------------------------
#define RVMODEL_IO_INIT(_R1, _R2, _R3)
#define RVMODEL_IO_WRITE_STR(_R1, _R2, _R3, _STR_PTR)

// ---------------------------------------------------------------------------
// Access fault address: below RAM base, guaranteed to fault on load/store.
// ---------------------------------------------------------------------------
#define RVMODEL_ACCESS_FAULT_ADDRESS 0x00000000

// ---------------------------------------------------------------------------
// Timer: no CLINT accessible from test code; leave blank.
// The CLINT is in the sail.json memory map for Sail's own use only.
// ---------------------------------------------------------------------------
#define RVMODEL_MTIME_ADDRESS    0x200bff8
#define RVMODEL_MTIMECMP_ADDRESS 0x2004000

// No timers

#define RVMODEL_TIMER_INT_SOON_DELAY

// ---------------------------------------------------------------------------
// mtvec alignment.
// ---------------------------------------------------------------------------
#define RVMODEL_MTVEC_ALIGN 2

// ---------------------------------------------------------------------------
// Interrupt stubs - no interrupt controller implemented.
// ---------------------------------------------------------------------------
#define RVMODEL_SET_MSW_INT(_R1, _R2)
#define RVMODEL_CLR_MSW_INT(_R1, _R2)
#define RVMODEL_SET_MEXT_INT(_R1, _R2)
#define RVMODEL_CLR_MEXT_INT(_R1, _R2)
#define RVMODEL_SET_SSW_INT(_R1, _R2)
#define RVMODEL_CLR_SSW_INT(_R1, _R2)
#define RVMODEL_SET_SEXT_INT(_R1, _R2)
#define RVMODEL_CLR_SEXT_INT(_R1, _R2)
#define RVMODEL_CLR_STIMER_INT(_R1, _R2)
// RVMODEL_SET_VSW_INT / RVMODEL_CLR_VSW_INT are intentionally NOT defined
// here: the framework invokes them bare, with no arguments (see
// rvtest_trap_handler.h's `clr_Vsw_int` label), unlike their MSW/SSW
// siblings. A function-like macro taking (_R1, _R2) never expands at a
// parenthesis-less call site, so defining them like the others left the
// literal macro name in the generated assembly and broke the build under
// STANDARD_SM_SUPPORTED. Leaving them undefined lets the framework's own
// `#ifndef` fallback (abort via RVTEST_DFLT_INT_HNDLR) apply, which is
// correct since this core doesn't implement the H extension and should
// never actually take a VS-mode software interrupt.
#define RVMODEL_INTERRUPT_LATENCY
