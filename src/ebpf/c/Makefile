OUTPUT := .output
CLANG ?= clang
CLANGXX ?= clang++
LIBBPF_SRC := $(abspath ../../../vendor/libbpf/src)
BPFTOOL_SRC := $(abspath ../../../vendor/bpftool/src)
LIBBPF_OBJ := $(abspath $(OUTPUT)/libbpf.a)
BPFTOOL_OUTPUT ?= $(abspath $(OUTPUT)/bpftool)
BPFTOOL ?= $(BPFTOOL_OUTPUT)/bootstrap/bpftool
ARCH ?= $(shell uname -m | sed 's/x86_64/x86/' \
			 | sed 's/arm.*/arm/' \
			 | sed 's/aarch64/arm64/' \
			 | sed 's/ppc64le/powerpc/' \
			 | sed 's/mips.*/mips/' \
			 | sed 's/riscv64/riscv/' \
			 | sed 's/loongarch64/loongarch/')
VMLINUX := ../../../vendor/vmlinux.h/include/$(ARCH)/vmlinux.h
INCLUDES := -I$(OUTPUT) -I../../../vendor/libbpf/include/uapi -I$(dir $(VMLINUX))
CFLAGS := -g -Wall
CXXFLAGS := -g -Wall
ALL_LDFLAGS := $(LDFLAGS) $(EXTRA_LDFLAGS)

# Get Clang's default includes for BPF target
CLANG_BPF_SYS_INCLUDES ?= $(shell $(CLANG) -v -E - </dev/null 2>&1 \
	| sed -n '/<...> search starts here:/,/End of search list./{ s| \(/.*\)|-idirafter \1|p }')

# Build verbosity
ifeq ($(V),1)
	Q =
	msg =
else
	Q = @
	msg = @printf '  %-8s %s%s\n' "$(1)" "$(patsubst $(abspath $(OUTPUT))/%,%,$(2))" "$(if $(3), $(3))";
	MAKEFLAGS += --no-print-directory
endif

# Allow compiler overrides
define allow-override
  $(if $(or $(findstring environment,$(origin $(1))),\
            $(findstring command line,$(origin $(1)))),,\
    $(eval $(1) = $(2)))
endef

$(call allow-override,CC,$(CROSS_COMPILE)cc)
$(call allow-override,LD,$(CROSS_COMPILE)ld)
$(call allow-override,CXX,$(CROSS_COMPILE)c++)

.PHONY: all clean typegen
all: libbootstrap.a example

clean:
	$(call msg,CLEAN)
	$(Q)rm -rf $(OUTPUT) libbootstrap.a example

typegen:
	$(call msg,TYPEGEN,bootstrap.gen.h)
	$(Q)cd ../typegen && cargo run

$(OUTPUT) $(OUTPUT)/libbpf $(BPFTOOL_OUTPUT):
	$(call msg,MKDIR,$@)
	$(Q)mkdir -p $@

# Build libbpf
$(LIBBPF_OBJ): $(wildcard $(LIBBPF_SRC)/*.[ch] $(LIBBPF_SRC)/Makefile) | $(OUTPUT)/libbpf
	$(call msg,LIB,$@)
	$(Q)$(MAKE) -C $(LIBBPF_SRC) BUILD_STATIC_ONLY=1 \
		    OBJDIR=$(dir $@)/libbpf DESTDIR=$(dir $@) \
		    INCLUDEDIR= LIBDIR= UAPIDIR= \
		    install

# Build bpftool
$(BPFTOOL): | $(BPFTOOL_OUTPUT)
	$(call msg,BPFTOOL,$@)
	$(Q)$(MAKE) ARCH= CROSS_COMPILE= OUTPUT=$(BPFTOOL_OUTPUT)/ \
		LIBBPF_PATH=$(abspath ../../../vendor) \
		-C $(BPFTOOL_SRC) bootstrap

# Build BPF code
$(OUTPUT)/bootstrap.bpf.o: bootstrap.bpf.c $(LIBBPF_OBJ) $(wildcard *.h) $(wildcard *.hpp) $(VMLINUX) | $(OUTPUT) $(BPFTOOL) typegen
	$(call msg,BPF,$@)
	$(Q)$(CLANG) -g -O2 -target bpf -D__TARGET_ARCH_$(ARCH) \
		$(INCLUDES) $(CLANG_BPF_SYS_INCLUDES) \
		-c $(filter %.c,$^) -o $(patsubst %.bpf.o,%.tmp.bpf.o,$@)
	$(Q)$(BPFTOOL) gen object $@ $(patsubst %.bpf.o,%.tmp.bpf.o,$@)

# Generate BPF skeleton
$(OUTPUT)/bootstrap.skel.h: $(OUTPUT)/bootstrap.bpf.o | $(OUTPUT) $(BPFTOOL)
	$(call msg,GEN-SKEL,$@)
	$(Q)$(BPFTOOL) gen skeleton $< > $@

# Build static library for Rust binding
$(OUTPUT)/bootstrap_lib.o: bootstrap.cpp $(OUTPUT)/bootstrap.skel.h $(wildcard *.h) $(wildcard *.hpp) | $(OUTPUT)
	$(call msg,CXX,$@)
	$(Q)$(CLANGXX) $(CXXFLAGS) $(INCLUDES) -c $(filter %.cpp,$^) -o $@

libbootstrap.a: $(OUTPUT)/bootstrap_lib.o $(LIBBPF_OBJ) | $(OUTPUT)
	$(call msg,STATICLIB,$@)
	$(Q)$(AR) rcs $@ $^

# Build example program (C++ version)
$(OUTPUT)/example.o: example.cpp $(wildcard *.h) $(wildcard *.hpp) | $(OUTPUT)
	$(call msg,CXX,$@)
	$(Q)$(CLANGXX) $(CXXFLAGS) $(INCLUDES) -c $(filter %.cpp,$^) -o $@

example: $(OUTPUT)/example.o libbootstrap.a | $(OUTPUT)
	$(call msg,BINARY,$@)
	$(Q)$(CLANGXX) $(CXXFLAGS) $< libbootstrap.a $(LIBBPF_OBJ) $(ALL_LDFLAGS) -lelf -lz -o $@

# delete failed targets
.DELETE_ON_ERROR:

# keep intermediate (.skel.h, .bpf.o, etc) targets
.SECONDARY: