# Makefile for Causm Verified Microkernel
CC        ?= clang
AR        ?= ar
CARGO     ?= cargo
FSTAR_EXE ?= fstar.exe
KRML_EXE  ?= krml
PYTHON    ?= python3

SRC_DIRS  := src/spec src/proof src/pulse
CACHE_DIR := .cache
HINTS_DIR := hints
DIST_DIR  := dist

CANDIDATE_ULIBS := \
	$(shell $(FSTAR_EXE) --locate 2>/dev/null) \
	$(HOME)/.opam/fstar/lib/fstar/ulib \
	/data/data/com.termux/files/home/.opam/fstar/lib/fstar/ulib \
	$(dir $(shell which $(FSTAR_EXE) 2>/dev/null))../lib/fstar/ulib

FSTAR_ULIB := $(firstword $(foreach d,$(CANDIDATE_ULIBS),$(if $(wildcard $(d)/Prims.fst),$(d))))

CANDIDATE_PULSE_LIBS := \
	$(HOME)/.opam/fstar/lib/fstar/pulse/pulse/lib \
	$(HOME)/.opam/fstar/lib/fstar/pulse/lib \
	/data/data/com.termux/files/home/.opam/fstar/pulse/pulse/lib \
	$(FSTAR_ULIB)/../pulse/pulse/lib \
	$(FSTAR_ULIB)/../pulse/lib

PULSE_LIB := $(firstword $(foreach d,$(CANDIDATE_PULSE_LIBS),$(if $(wildcard $(d)/Pulse.fst),$(d))))

CANDIDATE_KRML_INCLUDES := \
	$(shell $(KRML_EXE) -locate-include 2>/dev/null) \
	$(HOME)/.local/opt/karamel/include \
	/data/data/com.termux/files/home/.local/opt/karamel/include

KRML_INCLUDE := $(firstword $(foreach d,$(CANDIDATE_KRML_INCLUDES),$(if $(wildcard $(d)/krmllib.h $(d)/krml/krmllib.h),$(d))))

CANDIDATE_KRML_LIBS := \
	$(shell $(KRML_EXE) -locate-krmllib 2>/dev/null) \
	$(HOME)/.local/opt/karamel/krmllib \
	/data/data/com.termux/files/home/.local/opt/karamel/krmllib

KRML_LIB := $(firstword $(foreach d,$(CANDIDATE_KRML_LIBS),$(if $(wildcard $(d)/dist/minimal),$(d))))

C_INCLUDES := -I $(DIST_DIR) -I $(DIST_DIR)/internal \
	$(if $(KRML_INCLUDE),-I $(KRML_INCLUDE) -I $(KRML_INCLUDE)/krml) \
	$(if $(KRML_LIB),-I $(KRML_LIB) -I $(KRML_LIB)/dist/minimal) \
	$(if $(FSTAR_ULIB),-I $(FSTAR_ULIB)/c)

UNAME_M := $(shell uname -m)
ARCH_FLAGS :=
ifneq ($(filter aarch64 arm64,$(UNAME_M)),)
	ARCH_FLAGS += -march=armv8-a
endif

CFLAGS ?= -O3 -Wall -Wextra -Wno-bitwise-op-parentheses $(ARCH_FLAGS) $(C_INCLUDES)

.PHONY: all help build compile clean clean-c clean-rust distclean \
	test test-c test-rust verify verify-spec verify-proof verify-pulse \
	extract dirs

all: compile

help:
	@echo "Causm Verified Kernel Build System"
	@echo ""
	@echo "Build & Compilation:"
	@echo "  make compile               - Build verified static library (libcausm_kernel.a)"
	@echo ""
	@echo "Formal Verification:"
	@echo "  make verify                - Run full F* and Pulse verification suite"
	@echo "  make extract               - Extract verified C source via KaRaMeL"
	@echo ""
	@echo "Testing:"
	@echo "  make test                  - Run C integration tests and Rust FFI tests"

$(DIST_DIR)/%.o: $(DIST_DIR)/%.c
	@mkdir -p $(DIST_DIR)
	@echo "[CC]  $<"
	@$(CC) $(CFLAGS) -c $< -o $@

KERNEL_OBJS := $(DIST_DIR)/Pulse_Causm_Arena.o \
               $(DIST_DIR)/Pulse_Causm_Semantics.o \
               $(DIST_DIR)/Pulse_Causm_CFG.o \
               $(DIST_DIR)/Pulse_Causm_TypeChecker.o \
               $(DIST_DIR)/Spec_Causm_Lattice.o

$(DIST_DIR)/libcausm_kernel.a: $(KERNEL_OBJS)
	@echo "[AR]  $@"
	@if [ -f $(DIST_DIR)/krmlinit.o ]; then \
		$(AR) rcs $@ $^ $(DIST_DIR)/krmlinit.o; \
	else \
		$(AR) rcs $@ $^; \
	fi

compile: extract $(DIST_DIR)/libcausm_kernel.a
build: compile

tests/test_arena_decay: tests/test_arena_decay.c $(DIST_DIR)/libcausm_kernel.a
	@mkdir -p tests
	@$(CC) $(CFLAGS) $< $(DIST_DIR)/libcausm_kernel.a -o $@

test-c: tests/test_arena_decay
	@echo "==> Running C Kernel Decay & Arena Tests"
	@./tests/test_arena_decay

test-rust: compile
	@echo "==> Running causm-kernel-sys Tests"
	@$(CARGO) test --manifest-path bindings/causm-kernel-sys/Cargo.toml

test: compile
	@case "$(filter-out test,$(MAKECMDGOALS))" in \
		rust) $(MAKE) test-rust ;; \
		c) $(MAKE) test-c ;; \
		"") $(MAKE) test-c && $(MAKE) test-rust ;; \
		*) echo "Unknown test target: $(filter-out test,$(MAKECMDGOALS))"; exit 1 ;; \
	esac

rust:
	@:
c:
	@:

EXTRA_INCLUDES := $(if $(PULSE_LIB),--include $(PULSE_LIB)) --include $(CACHE_DIR)
INCLUDE_PATHS  := $(addprefix --include , $(SRC_DIRS)) $(EXTRA_INCLUDES)

FSTAR_FLAGS ?= $(INCLUDE_PATHS) \
	--cache_dir $(CACHE_DIR) \
	--odir $(DIST_DIR) \
	--cache_checked_modules \
	--warn_error -271-272-241 \
	--already_cached 'Prims,FStar,LowStar,PulseCore,Pulse'

FSTAR := $(FSTAR_EXE) $(FSTAR_FLAGS)

KRML_COMMON_FLAGS := \
	-skip-compilation \
	-minimal \
	-fbuiltin-uint128 \
	-fparentheses \
	-warn-error -2-4-9-15 \
	-library 'FStar.UInt128,FStar.Int.Cast.Full,FStar.Endianness' \
	-drop 'Prims,FStar,FStar.UInt128,FStar.UInt64,FStar.UInt32,FStar.UInt8,FStar.Int.*,FStar.SizeT,FStar.Seq.*,FStar.Ghost,LowStar.*,PulseCore.*' \
	-tmpdir $(DIST_DIR) \
	-I src/spec \
	-I src/proof \
	-I src/pulse \
	-I $(CACHE_DIR) \
	$(if $(PULSE_LIB),-I $(PULSE_LIB))

dirs:
	@mkdir -p $(CACHE_DIR)/spec $(CACHE_DIR)/proof $(CACHE_DIR)/pulse $(HINTS_DIR) $(DIST_DIR) tests bindings

SPEC_FILES  := $(wildcard src/spec/*.fst)
PROOF_FILES := $(wildcard src/proof/*.fst)
PULSE_FILES := $(wildcard src/pulse/*.fst)

# Flat checked targets in CACHE_DIR
$(CACHE_DIR)/%.fst.checked: src/spec/%.fst
	@mkdir -p $(CACHE_DIR)
	@echo "[VERIFY] $<"
	@$(FSTAR) $< && touch $@

$(CACHE_DIR)/%.fst.checked: src/proof/%.fst
	@mkdir -p $(CACHE_DIR)
	@echo "[VERIFY] $<"
	@$(FSTAR) $< && touch $@

$(CACHE_DIR)/%.fst.checked: src/pulse/%.fst
	@mkdir -p $(CACHE_DIR)
	@echo "[VERIFY] $<"
	@$(FSTAR) $< && touch $@

SPEC_CHECKED  := $(patsubst src/spec/%,$(CACHE_DIR)/%,$(SPEC_FILES:=.checked))
PROOF_CHECKED := $(patsubst src/proof/%,$(CACHE_DIR)/%,$(PROOF_FILES:=.checked))
PULSE_CHECKED := $(patsubst src/pulse/%,$(CACHE_DIR)/%,$(PULSE_FILES:=.checked))

verify-spec: dirs $(SPEC_CHECKED)
	@echo "[PASS] F* specifications verified."

verify-proof: dirs $(PROOF_CHECKED)
	@echo "[PASS] Formal proofs verified."

verify-pulse: dirs $(PULSE_CHECKED)
	@echo "[PASS] Pulse implementations verified."

verify: dirs $(SPEC_CHECKED) $(PROOF_CHECKED) $(PULSE_CHECKED)
	@echo "[PASS] Full formal verification suite complete."

# Extract single-module KaRaMeL ASTs individually (F* strictly allows 1 file per invocation)
$(DIST_DIR)/Spec_Causm_Lattice.krml: src/spec/Spec.Causm.Lattice.fst $(CACHE_DIR)/Spec.Causm.Lattice.fst.checked
	@mkdir -p $(DIST_DIR)
	@echo "[EXTRACT-KRML] $<"
	@$(FSTAR) --codegen krml --extract_module Spec.Causm.Lattice $<

$(DIST_DIR)/Pulse_Causm_Arena.krml: src/pulse/Pulse.Causm.Arena.fst $(CACHE_DIR)/Pulse.Causm.Arena.fst.checked
	@mkdir -p $(DIST_DIR)
	@echo "[EXTRACT-KRML] $<"
	@$(FSTAR) --codegen krml --extract_module Pulse.Causm.Arena $<

$(DIST_DIR)/Pulse_Causm_Semantics.krml: src/pulse/Pulse.Causm.Semantics.fst $(CACHE_DIR)/Pulse.Causm.Semantics.fst.checked
	@mkdir -p $(DIST_DIR)
	@echo "[EXTRACT-KRML] $<"
	@$(FSTAR) --codegen krml --extract_module Pulse.Causm.Semantics $<

$(DIST_DIR)/Pulse_Causm_CFG.krml: src/pulse/Pulse.Causm.CFG.fst $(CACHE_DIR)/Pulse.Causm.CFG.fst.checked
	@mkdir -p $(DIST_DIR)
	@echo "[EXTRACT-KRML] $<"
	@$(FSTAR) --codegen krml --extract_module Pulse.Causm.CFG $<

$(DIST_DIR)/Pulse_Causm_TypeChecker.krml: src/pulse/Pulse.Causm.TypeChecker.fst $(CACHE_DIR)/Pulse.Causm.TypeChecker.fst.checked
	@mkdir -p $(DIST_DIR)
	@echo "[EXTRACT-KRML] $<"
	@$(FSTAR) --codegen krml --extract_module Pulse.Causm.TypeChecker $<

$(DIST_DIR)/Pulse_Causm_Arena.c $(DIST_DIR)/Pulse_Causm_Semantics.c $(DIST_DIR)/Pulse_Causm_CFG.c $(DIST_DIR)/Pulse_Causm_TypeChecker.c $(DIST_DIR)/Spec_Causm_Lattice.c $(DIST_DIR)/krmlinit.c: $(DIST_DIR)/Pulse_Causm_Arena.krml $(DIST_DIR)/Pulse_Causm_Semantics.krml $(DIST_DIR)/Pulse_Causm_CFG.krml $(DIST_DIR)/Pulse_Causm_TypeChecker.krml $(DIST_DIR)/Spec_Causm_Lattice.krml
	@mkdir -p $(DIST_DIR)
	@cp -f include/causm_base.h $(DIST_DIR)/
	@echo "[EXTRACT] Running KaRaMeL extraction..."
	@$(KRML_EXE) $(KRML_COMMON_FLAGS) \
		-bundle 'Pulse_Causm_Arena=Pulse.Causm.Arena[rename=Pulse_Causm_Arena]' \
		-bundle 'Pulse_Causm_Semantics=Pulse.Causm.Semantics[rename=Pulse_Causm_Semantics]' \
		-bundle 'Pulse_Causm_CFG=Pulse.Causm.CFG[rename=Pulse_Causm_CFG]' \
		-bundle 'Pulse_Causm_TypeChecker=Pulse.Causm.TypeChecker[rename=Pulse_Causm_TypeChecker]' \
		-bundle 'Spec_Causm_Lattice=Spec.Causm.Lattice[rename=Spec_Causm_Lattice]' \
		-add-include '"causm_base.h"' \
		$(DIST_DIR)/Pulse_Causm_Arena.krml $(DIST_DIR)/Pulse_Causm_Semantics.krml $(DIST_DIR)/Pulse_Causm_CFG.krml $(DIST_DIR)/Pulse_Causm_TypeChecker.krml $(DIST_DIR)/Spec_Causm_Lattice.krml
	@echo "[DONE] C artifacts ready in $(DIST_DIR)/"

extract: $(DIST_DIR)/Pulse_Causm_Arena.c $(DIST_DIR)/Pulse_Causm_Semantics.c $(DIST_DIR)/Pulse_Causm_CFG.c $(DIST_DIR)/Pulse_Causm_TypeChecker.c $(DIST_DIR)/Spec_Causm_Lattice.c $(DIST_DIR)/krmlinit.c

clean-c:
	@echo "==> Cleaning C compilation artifacts"
	@rm -rf $(DIST_DIR)/*.o $(DIST_DIR)/*.a $(DIST_DIR)/*.krml $(DIST_DIR)/*.c $(DIST_DIR)/*.h $(DIST_DIR)/internal
	@rm -f tests/test_arena_decay

clean-rust:
	@echo "==> Running cargo clean on Rust crates"
	@$(CARGO) clean --manifest-path bindings/causm-kernel-sys/Cargo.toml

clean:
	@case "$(filter-out clean,$(MAKECMDGOALS))" in \
		rust) $(MAKE) clean-rust ;; \
		c) $(MAKE) clean-c ;; \
		"") $(MAKE) clean-c && $(MAKE) clean-rust ;; \
		*) echo "Unknown clean target: $(filter-out clean,$(MAKECMDGOALS))"; exit 1 ;; \
	esac

distclean: clean
	@rm -rf $(CACHE_DIR) $(DIST_DIR) .depend
