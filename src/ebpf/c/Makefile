# Directory for artefacts
OUTPUT          ?= .output

# Toolchain
CLANG           ?= clang
CLANGXX         ?= clang++
STACKPROTECT_FLAG := -fno-stack-protector

# External sources
LIBBPF_SRC      := $(abspath ../../../vendor/libbpf/src)
BPFTOOL_SRC     := $(abspath ../../../vendor/bpftool/src)

LIBBPF_OBJ      := $(abspath $(OUTPUT)/libbpf.a)
BPFTOOL_OUTPUT  ?= $(abspath $(OUTPUT)/bpftool)
BPFTOOL         ?= $(BPFTOOL_OUTPUT)/bootstrap/bpftool

# Arch detection → vmlinux.h include location
ARCH            ?= $(shell uname -m | sed 's/x86_64/x86/' \
                             | sed 's/arm.*/arm/' \
                             | sed 's/aarch64/arm64/' \
                             | sed 's/ppc64le/powerpc/' \
                             | sed 's/mips.*/mips/' \
                             | sed 's/riscv64/riscv/' \
                             | sed 's/loongarch64/loongarch/')
VMLINUX         := ../../../vendor/vmlinux.h/include/$(ARCH)/vmlinux.h

# Flags
CPPFLAGS        += -I$(OUTPUT) -I../../../vendor/libbpf/include/uapi -I$(dir $(VMLINUX))
CFLAGS          ?= -g -Wall
CXXFLAGS        ?= -g -Wall
override CFLAGS   += $(STACKPROTECT_FLAG) -D_GNU_SOURCE
override CXXFLAGS += $(STACKPROTECT_FLAG) -D_GNU_SOURCE
CLANG_BPF_SYS_INCLUDES ?= $(shell $(CLANG) -v -E - </dev/null 2>&1 \
	| sed -n '/<...> search starts here:/,/End of search list./{ s| \(/.*\)|-idirafter \1|p }')

ALL_LDFLAGS     := $(LDFLAGS) $(EXTRA_LDFLAGS)

# Make quiet vs verbose
ifeq ($(V),1)
	Q :=
	msg :=
else
	Q := @
	msg = @printf '  %-10s %s%s\n' "$(1)" "$(patsubst $(abspath $(OUTPUT))/%,%,$(2))" "$(if $(3), $(3))";
	MAKEFLAGS += --no-print-directory
endif

# Allow CROSS_COMPILE overrides
define allow-override
  $(if $(or $(findstring environment,$(origin $(1))),\
            $(findstring command line,$(origin $(1)))),,\
    $(eval $(1) = $(2)))
endef
$(call allow-override,CC,$(CROSS_COMPILE)cc)
$(call allow-override,LD,$(CROSS_COMPILE)ld)
$(call allow-override,CXX,$(CROSS_COMPILE)c++)

# ─────────────────────────────────────────────────────────────────────────────
# Targets
# ─────────────────────────────────────────────────────────────────────────────
.PHONY: all clean codegen
all: typegen libbootstrap.a example

clean:
	$(call msg,CLEAN)
	$(Q)rm -rf $(OUTPUT) libbootstrap.a example

# Code generation from TOML config
typegen:
	$(call msg,CODEGEN,typegen)
	$(Q)cd ../typegen && cargo run

# ­­­create directories
$(OUTPUT) $(OUTPUT)/libbpf $(BPFTOOL_OUTPUT):
	$(call msg,MKDIR,$@)
	$(Q)mkdir -p $@

# libbpf
$(LIBBPF_OBJ): $(wildcard $(LIBBPF_SRC)/*.[ch] $(LIBBPF_SRC)/Makefile) | $(OUTPUT)/libbpf
	$(call msg,LIB,$@)
	$(Q)$(MAKE) -C $(LIBBPF_SRC) \
		BUILD_STATIC_ONLY=1 \
		EXTRA_CFLAGS="$(STACKPROTECT_FLAG)" \
		OBJDIR=$(dir $@)/libbpf DESTDIR=$(dir $@) \
		INCLUDEDIR= LIBDIR= UAPIDIR= \
		install

# bpftool
$(BPFTOOL): | $(BPFTOOL_OUTPUT)
	$(call msg,BPFTOOL,$@)
	$(Q)$(MAKE) ARCH= OUTPUT=$(BPFTOOL_OUTPUT)/ \
		LIBBPF_PATH=$(abspath ../../../vendor) \
		-C $(BPFTOOL_SRC) bootstrap \
		EXTRA_CFLAGS="$(STACKPROTECT_FLAG)"

# BPF object (kernel byte-code)
$(OUTPUT)/bootstrap.bpf.o: bootstrap.bpf.c $(LIBBPF_OBJ) $(wildcard *.h) $(wildcard *.hpp) $(VMLINUX) | $(OUTPUT) $(BPFTOOL)
	$(call msg,BPF,$@)
	$(Q)$(CLANG) -g -O2 -target bpf -D__TARGET_ARCH_$(ARCH) \
		$(STACKPROTECT_FLAG) $(CPPFLAGS) $(CLANG_BPF_SYS_INCLUDES) \
		-c $< -o $(patsubst %.bpf.o,%.tmp.bpf.o,$@)
	$(Q)$(BPFTOOL) gen object $@ $(patsubst %.bpf.o,%.tmp.bpf.o,$@)

# Skeleton
$(OUTPUT)/bootstrap.skel.h: $(OUTPUT)/bootstrap.bpf.o | $(OUTPUT) $(BPFTOOL)
	$(call msg,GEN-SKEL,$@)
	$(Q)$(BPFTOOL) gen skeleton $< > $@

# C wrapper library
$(OUTPUT)/bootstrap_lib.o: bootstrap.c $(OUTPUT)/bootstrap.skel.h $(wildcard *.h) | $(OUTPUT)
	$(call msg,CC,$@)
	$(Q)$(CLANG) $(CFLAGS) $(CPPFLAGS) -std=c11 -c $< -o $@

libbootstrap.a: $(OUTPUT)/bootstrap_lib.o | $(OUTPUT)
	$(call msg,AR,$@)
	$(Q)$(AR) rcs $@ $^

# Stand-alone C++ example
$(OUTPUT)/example.o: example.cpp $(wildcard *.h) $(wildcard *.hpp) | $(OUTPUT)
	$(call msg,CXX,$@)
	$(Q)$(CLANGXX) $(CXXFLAGS) $(CPPFLAGS) -c $< -o $@

example: $(OUTPUT)/example.o libbootstrap.a | $(OUTPUT)
	$(call msg,LINK,$@)
	$(Q)$(CLANGXX) $(CXXFLAGS) $< libbootstrap.a $(LIBBPF_OBJ) \
		$(ALL_LDFLAGS) -lelf -lz -o $@

# Keep intermediates (.skel.h, .tmp.bpf.o, …)
.SECONDARY:
.DELETE_ON_ERROR:
