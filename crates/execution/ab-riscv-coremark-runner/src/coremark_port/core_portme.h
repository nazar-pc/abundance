/*
Copyright 2018 Embedded Microprocessor Benchmark Consortium (EEMBC)

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.

Original Author: Shay Gal-on
*/
/* Topic : Description
        This file contains configuration constants required to execute on
   the Abundance RISC-V interpreter platform.
*/
#ifndef CORE_PORTME_H
#define CORE_PORTME_H
/************************/
/* Data types and settings */
/************************/
/* Configuration : HAS_FLOAT
        No hardware FPU; Coremark uses float only for score reporting,
        handled via integer arithmetic in ee_printf.
*/
#ifndef HAS_FLOAT
#define HAS_FLOAT 0
#endif
/* Configuration : HAS_TIME_H
        No time.h available on bare-metal guest.
*/
#ifndef HAS_TIME_H
#define HAS_TIME_H 0
#endif
/* Configuration : USE_CLOCK
        No clock() available; timing is done via rdtime CSR.
*/
#ifndef USE_CLOCK
#define USE_CLOCK 0
#endif
/* Configuration : HAS_STDIO
        No stdio.h available on bare-metal guest.
*/
#ifndef HAS_STDIO
#define HAS_STDIO 0
#endif
/* Configuration : HAS_PRINTF
        ee_printf is implemented in core_portme.c.
*/
#ifndef HAS_PRINTF
#define HAS_PRINTF 0
#endif

/* Definitions : COMPILER_VERSION, COMPILER_FLAGS, MEM_LOCATION
        Initialize these strings per platform
*/
#ifndef COMPILER_VERSION
#ifdef __GNUC__
#define COMPILER_VERSION "GCC"__VERSION__
#else
#define COMPILER_VERSION "Please put compiler version here (e.g. gcc 4.1)"
#endif
#endif
#ifndef COMPILER_FLAGS
#define COMPILER_FLAGS "-O3 -march=rv64imc_zba_zbb_zbs -mabi=lp64"
#endif
#ifndef MEM_LOCATION
#define MEM_LOCATION "STACK"
#endif

/* Data Types :
        To avoid compiler issues, define the data types that need to be used for
   8b, 16b and 32b in <core_portme.h>.

        *Important* :
        ee_ptr_int needs to be the data type used to hold pointers, otherwise
   coremark may fail!!!
*/
typedef signed short ee_s16;
typedef unsigned short ee_u16;
typedef signed int ee_s32;
typedef double ee_f32;
typedef unsigned char ee_u8;
typedef unsigned int ee_u32;
typedef unsigned long ee_ptr_int;
typedef unsigned long ee_size_t;
#define NULL ((void *)0)
/* align_mem :
        This macro is used to align an offset to point to a 32b value. It is
   used in the Matrix algorithm to initialize the input memory blocks.
*/
#define align_mem(x) (void *)(4 + (((ee_ptr_int)(x)-1) & ~3))

/* Configuration : CORE_TICKS
        Using rdtime CSR which returns nanoseconds; CORETIMETYPE must be
        wide enough to hold a 64-bit nanosecond timestamp.
*/
#define CORETIMETYPE ee_u64
typedef unsigned long long ee_u64;
typedef ee_u64 CORE_TICKS;

/* Timing : CLOCKS_PER_SEC
        barebones_clock() returns microseconds elapsed since execution start.
*/
#define CLOCKS_PER_SEC 1000000ULL

/* Configuration : ITERATIONS
        0 = auto-select based on timing (minimum 10 seconds for a valid run).
*/
#ifndef ITERATIONS
#define ITERATIONS 0
#endif

/* Configuration : SEED_METHOD
        Seeds come from volatile variables defined in core_portme.c.
*/
#ifndef SEED_METHOD
#define SEED_METHOD SEED_VOLATILE
#endif

/* Configuration : MEM_METHOD
        Stack allocation; Coremark declares its working set as a local in
        iterate(), no malloc needed.
*/
#ifndef MEM_METHOD
#define MEM_METHOD MEM_STACK
#endif

/* Configuration : MULTITHREAD
        Single context only.
*/
#ifndef MULTITHREAD
#define MULTITHREAD 1
#define USE_PTHREAD 0
#define USE_FORK    0
#define USE_SOCKET  0
#endif

/* Configuration : MAIN_HAS_NOARGC
        argc/argv are used to receive the output buffer pointer from the host.
*/
#ifndef MAIN_HAS_NOARGC
#define MAIN_HAS_NOARGC 0
#endif

/* Configuration : MAIN_HAS_NORETURN
        main returns normally.
*/
#ifndef MAIN_HAS_NORETURN
#define MAIN_HAS_NORETURN 0
#endif

/* Variable : default_num_contexts
        Not used for this simple port, must contain the value 1.
*/
extern ee_u32 default_num_contexts;

typedef struct CORE_PORTABLE_S
{
        ee_u8 portable_id;
} core_portable;

/* target specific init/fini */
void portable_init(core_portable* p, int* argc, char* argv[]);
void portable_fini(core_portable* p);

#if !defined(PROFILE_RUN) && !defined(PERFORMANCE_RUN) \
    && !defined(VALIDATION_RUN)
#if (TOTAL_DATA_SIZE == 1200)
#define PROFILE_RUN 1
#elif (TOTAL_DATA_SIZE == 2000)
#define PERFORMANCE_RUN 1
#else
#define VALIDATION_RUN 1
#endif
#endif

/* Output buffer : the host allocates this buffer and passes its address to
        the guest via argv[0] (cast to ee_u8*). portable_init stores it in
        output_buf. uart_send_char appends to it. The host reads a
        null-terminated string from the same allocation after execution ends.
*/
#define OUTPUT_BUF_MAX 4096u

extern volatile ee_u8* output_buf;

/* uart_send_char is called by ee_printf.c for each output character */
void uart_send_char(char c);

int ee_printf(const char* fmt, ...);

#endif /* CORE_PORTME_H */
