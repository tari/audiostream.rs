#include <stdint.h>

void do_cpuid(uint32_t query, uint32_t *outputs) {
    // EAX parameter
    outputs[0] = query;
    __asm__(
        "cpuid"
        : "+a"(outputs[0]), "=b"(outputs[1]), "=c"(outputs[2]), "=d"(outputs[3])
    );
}
