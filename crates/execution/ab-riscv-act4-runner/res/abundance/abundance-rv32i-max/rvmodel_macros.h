// rvmodel_macros.h - abundance-rv32i-max (ACT4 self-checking framework)
//
// Halt mechanism: HTIF tohost write (Sail's exit protocol).
//   Writing (exit_code << 1) | 1 to tohost signals exit.
//   Code 0 = pass: write 1. Code != 0 = fail: write (code<<1)|1.
//
// No console, no timer, no interrupts implemented.

// ---------------------------------------------------------------------------
// This core is M-mode only (no S/U), but does implement a real, writable
// mtvec and dispatches traps (ecall, illegal instruction, ...) through it -
// see AbundanceRv32IMaxExtState::take_trap() in the interpreter. Defining
// this lets the framework install and use its own real trap handler
// (RVTEST_TRAP_HANDLER), instead of leaving tests that rely on it (e.g. any
// RVTEST_GOTO_MMODE user, or extensions - like Zkr - whose tests are
// registered under priv/) with no working trap handler at all. Every other
// DUT config in this framework defines this the same way, leaving
// RVMODEL_BOOT undefined below.
// ---------------------------------------------------------------------------
#define STANDARD_SM_SUPPORTED

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

# Perform boot operations. Can be empty or left undefined unless needed for
# DUT-specific behavior such as turning on a memory controller or
# initializing custom state.
//#define RVMODEL_BOOT

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
// Timer: no CLINT accessible from test code. RVMODEL_MTIME_ADDRESS is
// intentionally left undefined - it is not required (see check_defines.h:
// "If RVMODEL_MTIME_ADDRESS is not defined, no machine timer interrupts are
// tested"), and defining it would make RVTEST_TRAP_PROLOG (under
// STANDARD_SM_SUPPORTED) write to RVMODEL_MTIMECMP_ADDRESS during boot,
// which is not backed by real memory in this DUT's memory map (only
// RAM_BASE..RAM_BASE+RAM_SIZE is, see main.rs) and would fault. The CLINT
// address in sail.json is for Sail's own reference-model use only.
// ---------------------------------------------------------------------------

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
