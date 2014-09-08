.PHONY: all

all: $(OUT_DIR)/libcpuid.a

$(OUT_DIR)/cpuid.o: src/cpuid.c
	cc -c src/cpuid.c -o $(OUT_DIR)/cpuid.o

$(OUT_DIR)/libcpuid.a: $(OUT_DIR)/cpuid.o
	ar rcs $(OUT_DIR)/libcpuid.a $(OUT_DIR)/cpuid.o
