// rvtest_config.h - abundance-rv32i-max
//
// Declares which optional features are present so the test framework can
// enable/disable the relevant test cases at compile time.
//
// RVMODEL_ACCESS_FAULT_ADDRESS: an address that always generates an access
//   fault when accessed. The interpreter rejects anything below BASE_ADDR
//   (0x80000000), so 0x0 is a safe choice - the halt address also lives
//   there, but the tests only need it to fault on load/store, not fetch.
//
// RVMODEL_PMP_GRAIN: log2(PMP granularity) - 2. No PMP implemented → 0.
// RVMODEL_NUM_PMPS: 0 = no PMP.
//
// *_SUPPORTED macros gate optional-extension test cases. Only declare
// what the interpreter actually implements.

#define RVMODEL_ACCESS_FAULT_ADDRESS 0x00000000
#define RVMODEL_PMP_GRAIN 0
#define RVMODEL_NUM_PMPS 0

#define B_SUPPORTED
#define M_SUPPORTED
#define V_SUPPORTED
#define ZBA_SUPPORTED
#define ZBB_SUPPORTED
#define ZBC_SUPPORTED
#define ZBKB_SUPPORTED
#define ZBKC_SUPPORTED
#define ZBKX_SUPPORTED
#define ZBS_SUPPORTED
#define ZICOND_SUPPORTED
#define ZICSR_SUPPORTED
#define ZKN_SUPPORTED
#define ZKND_SUPPORTED
#define ZKNE_SUPPORTED
#define ZKNH_SUPPORTED
#define ZMMUL_SUPPORTED
