#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <assert.h>
#include "Pulse_Causm_Arena.h"
#include "Pulse_Causm_Semantics.h"
#include "Pulse_Causm_CFG.h"
#include "Pulse_Causm_TypeChecker.h"
#include "Spec_Causm_Lattice.h"

int main(void) {
    printf("==> Running Causm Microkernel C Verification Suite...\n");

    // 1. Test Arena Allocation & Initialization
    printf("[1/4] Testing Arena & Entropic Diagnostics...\n");
    Pulse_Causm_Arena_cell_t cells[4];
    for (size_t i = 0; i < 4; i++) {
        cells[i] = (Pulse_Causm_Arena_cell_t){ .payload = 0, .tag = 3, .expire = 0 };
    }

    assert(Pulse_Causm_Arena_check_cell_access(cells, 0, 0) == 1); // UseAfterConsume
    Pulse_Causm_Arena_arena_init_valid(cells, 0, 42);
    assert(Pulse_Causm_Arena_check_cell_access(cells, 0, 0) == 0); // Ok
    assert(Pulse_Causm_Arena_arena_read(cells, 0, 0) == 42);

    Pulse_Causm_Arena_arena_lease(cells, 0, 10, 20); // expires at 30
    assert(Pulse_Causm_Arena_check_cell_access(cells, 0, 15) == 0); // Active lease Ok
    assert(Pulse_Causm_Arena_check_cell_access(cells, 0, 30) == 3); // LeaseExpired

    Pulse_Causm_Arena_arena_tick_decay(cells, 0, 30);
    assert(Pulse_Causm_Arena_check_cell_access(cells, 0, 30) == 2); // UseAfterDecay
    printf("      [PASS] Arena entropic lifecycle verified.\n");

    // 2. Test Entanglement Cascade
    printf("[2/4] Testing Entanglement Cascade Consumption...\n");
    uint8_t ent_matrix[16] = {0};
    for (size_t i = 0; i < 4; i++) {
        Pulse_Causm_Arena_arena_init_valid(cells, i, (i + 1) * 100);
    }
    Pulse_Causm_Semantics_exec_entangle(cells, ent_matrix, 4, 0, 1);
    assert(Pulse_Causm_Semantics_is_entangled(ent_matrix, 4, 0, 1));
    assert(Pulse_Causm_Semantics_is_entangled(ent_matrix, 4, 1, 0));

    assert(Pulse_Causm_Semantics_exec_consume_cascading(cells, ent_matrix, 4, 0));
    assert(cells[0].tag == 3);
    assert(cells[1].tag == 3); // Cascaded consume!
    assert(cells[2].tag == 0); // Not entangled, remained Valid
    printf("      [PASS] Entanglement graph cascade verified.\n");

    // 3. Test CFG Step & Execution
    printf("[3/4] Testing CFG Basic Block Execution...\n");
    Pulse_Causm_Arena_arena_init_valid(cells, 0, 10);
    Pulse_Causm_Arena_arena_init_valid(cells, 1, 20);
    uint64_t clk = 0;
    Pulse_Causm_CFG_raw_instr_t body[1] = {
        { .op = 1, .arg1 = 2, .arg2 = 0, .arg3 = 1, .imm = 0 } // Add R2, R0, R1
    };
    assert(Pulse_Causm_CFG_exec_body_loop(cells, ent_matrix, 4, &clk, body, 1, 0));
    assert(cells[2].payload == 30);
    assert(cells[2].tag == 0);
    printf("      [PASS] Basic block execution verified.\n");

    // 4. Test Static Type Checking
    printf("[4/4] Testing Static Type Checker Invariant Rejection...\n");
    Pulse_Causm_CFG_raw_instr_t invalid_body[1] = {
        { .op = 1, .arg1 = 3, .arg2 = 0, .arg3 = 1, .imm = 0 } // R0 or R1 consumed -> rejected
    };
    Pulse_Causm_Arena_arena_consume(cells, 0);
    assert(!Pulse_Causm_TypeChecker_verify_body_typing(cells, ent_matrix, 4, &clk, invalid_body, 1, 0));
    printf("      [PASS] Static type checker rejection verified.\n");

    printf("\nAll Causm C kernel verification tests PASSED with zero errors.\n");
    return 0;
}
