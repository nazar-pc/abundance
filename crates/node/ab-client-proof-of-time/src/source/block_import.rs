use ab_core_primitives::pot::{PotOutput, PotParametersChange, SlotNumber};

/// PoT information of the best block
#[derive(Debug, Copy, Clone)]
pub struct BestBlockPotInfo {
    /// Slot for which PoT output was generated
    pub slot: SlotNumber,
    /// PoT output itself
    pub pot_output: PotOutput,
    /// Change of parameters to apply to PoT chain
    pub pot_parameters_change: Option<PotParametersChange>,
}
