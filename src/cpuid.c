#include <stdint.h>

void do_cpuid(uint32_t eax, uint32_t ecx, uint32_t *outputs) {
    // EAX parameter
    outputs[0] = eax;
    outputs[2] = ecx;
    __asm__(
        "cpuid"
        : "+a"(outputs[0]), "=b"(outputs[1]), "+c"(outputs[2]), "=d"(outputs[3])
    );
}

uint64_t do_xgetbv(uint32_t ecx) {
    uint32_t low, high;
    __asm__(
        "xgetbv"
        : "=d"(high), "=a"(low)
        : "c"(ecx)
    );
    return ((uint64_t)high << 32) | low;
}
