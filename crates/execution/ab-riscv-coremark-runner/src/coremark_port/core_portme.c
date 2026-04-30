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
#include "coremark.h"
#include "core_portme.h"

#if VALIDATION_RUN
volatile ee_s32 seed1_volatile = 0x3415;
volatile ee_s32 seed2_volatile = 0x3415;
volatile ee_s32 seed3_volatile = 0x66;
#endif
#if PERFORMANCE_RUN
volatile ee_s32 seed1_volatile = 0x0;
volatile ee_s32 seed2_volatile = 0x0;
volatile ee_s32 seed3_volatile = 0x66;
#endif
#if PROFILE_RUN
volatile ee_s32 seed1_volatile = 0x8;
volatile ee_s32 seed2_volatile = 0x8;
volatile ee_s32 seed3_volatile = 0x8;
#endif
volatile ee_s32 seed4_volatile = ITERATIONS;
volatile ee_s32 seed5_volatile = 0;

/* Porting : Timing functions
    The RISC-V unprivileged `time` CSR (0xC01) is mapped by the host interpreter to nanoseconds elapsed since an
    arbitrary point in time. Converted to microseconds here, so CLOCKS_PER_SEC is therefore 1e6.
    */
CORETIMETYPE
barebones_clock()
{
    ee_u64 t;
    __asm__ volatile("rdtime %0" : "=r"(t));
    // Convert ns to µs to survive integer division in time_in_secs
    return t / 1000;
}

#define GETMYTIME(_t)              (*(_t) = barebones_clock())
#define MYTIMEDIFF(fin, ini)       ((fin) - (ini))
#define TIMER_RES_DIVIDER          1
#define SAMPLE_TIME_IMPLEMENTATION 1
#define EE_TICKS_PER_SEC           (CLOCKS_PER_SEC / TIMER_RES_DIVIDER)

/** Define Host specific (POSIX), or target specific global time variables. */
static CORETIMETYPE start_time_val, stop_time_val;

/* Function : start_time
        This function will be called right before starting the timed portion of
   the benchmark.
*/
void
start_time(void)
{
    GETMYTIME(&start_time_val);
}

/* Function : stop_time
        This function will be called right after ending the timed portion of
   the benchmark.
*/
void
stop_time(void)
{
    GETMYTIME(&stop_time_val);
}

/* Function : get_time
        Return an abstract "ticks" number that signifies time on the system.
*/
CORE_TICKS
get_time(void)
{
    CORE_TICKS elapsed
        = (CORE_TICKS)(MYTIMEDIFF(stop_time_val, start_time_val));
    return elapsed;
}

/* Function : time_in_secs
        Convert the value returned by get_time to seconds.
*/
secs_ret
time_in_secs(CORE_TICKS ticks)
{
    secs_ret retval = ((secs_ret)ticks) / (secs_ret)EE_TICKS_PER_SEC;
    return retval;
}

ee_u32 default_num_contexts = 1;

/* Output buffer */

/*
 * output_buf_storage is placed in its own ELF section so the host can discover both the guest address and the capacity
 * (section size) without any hardcoded constants. The host reads the ".output_buf" section address and passes it to the
 * guest via a1/argv[0]; portable_init stores it in output_buf. uart_send_char appends to it; the host reads a
 * null-terminated string from the same address after execution ends.
 */
__attribute__((section(".output_buf")))
volatile ee_u8 output_buf_storage[OUTPUT_BUF_MAX];

volatile ee_u8* output_buf;
static ee_size_t output_pos;

/* Function : uart_send_char
        Called by ee_printf for each output character.
*/
void
uart_send_char(char c)
{
    if (output_buf != (volatile ee_u8*)NULL && output_pos < OUTPUT_BUF_MAX - 1u)
    {
        output_buf[output_pos++] = (ee_u8)c;
        output_buf[output_pos] = 0;
    }
}

/* Function : portable_init
        Target specific initialization code.

        argv[0] is cast to (ee_u8 *) and used as the output buffer address.
        The host sets a1 = output buffer guest address before calling main,
        which the C runtime delivers here as argv[0].
*/
void
portable_init(core_portable* p, int* argc, char* argv[])
{
    if (sizeof(ee_ptr_int) != sizeof(ee_u8*))
    {
        ee_printf(
            "ERROR! Please define ee_ptr_int to a type that holds a "
            "pointer!\n");
    }
    if (sizeof(ee_u32) != 4)
    {
        ee_printf("ERROR! Please define ee_u32 to a 32b unsigned type!\n");
    }
    output_buf = (volatile ee_u8*)(ee_ptr_int)argv[0];
    output_pos = 0;
    p->portable_id = 1;
}

/* Function : portable_fini
        Target specific final code
*/
void
portable_fini(core_portable* p)
{
    p->portable_id = 0;
}
