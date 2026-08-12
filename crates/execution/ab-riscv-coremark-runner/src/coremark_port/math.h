/* Minimal <math.h> for the bare-metal guest.
 *
 * Coremark's `barebones/cvt.c` is the only consumer and needs exactly one function. It is compiled
 * with `-ffreestanding -nostdlib` against a toolchain that may ship no newlib at all, so providing
 * `modf` here avoids depending on a libm being present.
 */
#ifndef COREMARK_PORT_MATH_H
#define COREMARK_PORT_MATH_H

/* Split into integral and fractional parts, truncating toward zero. Coremark only formats scores,
 * which are far below the range where the `long long` round trip loses anything. */
static inline double
modf(double value, double *integral_part)
{
    double integral = (double)(long long)value;
    *integral_part  = integral;
    return value - integral;
}

#endif
