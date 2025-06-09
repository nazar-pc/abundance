import math
from scipy.stats import binom
from prettytable import PrettyTable

def calculate_attack_probability(K_M, N, k_alpha):
    """
    Calculates the probability of an attacker getting at least k_alpha plots
    in a specific shard, given their total plots K_M and total shards N.
    """
    if k_alpha <= 0 or K_M <= 0 or N <= 0:
        return 0.0 # Invalid input
    if k_alpha > K_M:
        return 0.0 # Attacker doesn't have enough plots in total

    p_success = 1.0 / N
    probability = binom.sf(k_alpha - 1, K_M, p_success)
    return probability

def evaluate_scenario(
    num_shards,
    max_plot_size_gib,
    total_network_storage_pib,
    plotting_throughput_tib_hr_per_attacker,
    attacker_storage_percentage,
    cost_per_plot_usd, # NEW PARAMETER: Economic cost to create one plot
    control_percentage_for_shard=0.51, # 51% for longest chain
    target_reshuffle_interval_hours=1,
    avg_block_time_seconds=5,
    avg_block_size_mb=1
):
    """
    Evaluates a single scenario and returns key metrics for concise display.
    """
    total_network_storage_gib = total_network_storage_pib * 1024 * 1024
    attacker_plotting_throughput_gib_hr = plotting_throughput_tib_hr_per_attacker * 1024

    # Shard Capacity and k_alpha
    target_storage_per_shard_gib = total_network_storage_gib / num_shards
    plots_per_shard_capacity = math.floor(target_storage_per_shard_gib / max_plot_size_gib)
    k_alpha = math.ceil(control_percentage_for_shard * plots_per_shard_capacity)
    if k_alpha == 0: # Ensure k_alpha is at least 1 if control is desired
        k_alpha = 1

    # Attacker's Total Plots (K_M)
    attacker_total_storage_gib = total_network_storage_gib * attacker_storage_percentage
    K_M = math.floor(attacker_total_storage_gib / max_plot_size_gib)

    # Probability of Plotting Attack
    prob_plotting_attack = calculate_attack_probability(K_M, num_shards, k_alpha)

    # Time for Attacker to Plot KM plots
    plots_per_hr_per_attacker = attacker_plotting_throughput_gib_hr / max_plot_size_gib
    time_to_plot_km_hours = K_M / plots_per_hr_per_attacker if plots_per_hr_per_attacker > 0 else float('inf')

    # Economic Cost of Attack
    economic_cost_of_attack_usd = K_M * cost_per_plot_usd

    # Cost of Reshuffling (Data to Sync)
    blocks_per_shard_per_hour = (target_reshuffle_interval_hours * 3600) / avg_block_time_seconds
    data_to_sync_per_shard_per_hour_gb = (blocks_per_shard_per_hour * avg_block_size_mb) / 1024 # MB to GB

    # Qualitative Assessment
    plotting_attack_risk = "Low (Time to plot >> Reshuffle)"
    if time_to_plot_km_hours <= target_reshuffle_interval_hours:
        plotting_attack_risk = "High (Time to plot <= Reshuffle)"
    elif time_to_plot_km_hours <= target_reshuffle_interval_hours * 24: # Within 24 hours
        plotting_attack_risk = "Medium (Time to plot ~ Day)"

    allocation_prob_risk = "Very Low"
    if prob_plotting_attack > 0.0001: # 0.01%
        allocation_prob_risk = "Moderate"
    if prob_plotting_attack > 0.01: # 1%
        allocation_prob_risk = "High"

    sync_feasibility = "Manageable"
    if data_to_sync_per_shard_per_hour_gb > 10: # >10 GB/hr is quite high for casual users
        sync_feasibility = "Demanding"
    elif data_to_sync_per_shard_per_hour_gb > 25:
        sync_feasibility = "Very Demanding"


    return {
        "N (Shards)": num_shards,
        "Net Storage (PiB)": total_network_storage_pib,
        "Plot Size (GiB)": max_plot_size_gib,
        "Attacker %": f"{attacker_storage_percentage:.1%}",
        "k_alpha (plots)": k_alpha,
        "K_M (plots)": K_M,
        "Plotting Prob.": f"{prob_plotting_attack:.2e}", # Scientific notation
        "Plot Time (days)": f"{time_to_plot_km_hours / 24:.2f}",
        "Economic Cost (USD)": f"${economic_cost_of_attack_usd:,.0f}", # Formatted as currency
        "Reshuffle (hrs)": target_reshuffle_interval_hours,
        "Sync Data (GB/hr)": f"{data_to_sync_per_shard_per_hour_gb:.2f}",
        "Plot Attack Risk": plotting_attack_risk,
        "Alloc. Prob. Risk": allocation_prob_risk,
        "Sync Feasibility": sync_feasibility
    }

# --- Define Scenarios ---
scenarios = [
    {
        "name": "S1: Large Network, Low Attacker",
        "num_shards": 100,
        "max_plot_size_gib": 100,
        "total_network_storage_pib": 500,
        "plotting_throughput_tib_hr_per_attacker": 3.6,
        "attacker_storage_percentage": 0.01, # 1%
        "cost_per_plot_usd": 0.25, # New parameter
        "target_reshuffle_interval_hours": 1
    },
    {
        "name": "S2: Large Network, Med Attacker",
        "num_shards": 100,
        "max_plot_size_gib": 100,
        "total_network_storage_pib": 500,
        "plotting_throughput_tib_hr_per_attacker": 3.6,
        "attacker_storage_percentage": 0.05, # 5%
        "cost_per_plot_usd": 0.25,
        "target_reshuffle_interval_hours": 1
    },
    {
        "name": "S3: Large Network, High Attacker",
        "num_shards": 100,
        "max_plot_size_gib": 100,
        "total_network_storage_pib": 500,
        "plotting_throughput_tib_hr_per_attacker": 3.6,
        "attacker_storage_percentage": 0.10, # 10%
        "cost_per_plot_usd": 0.25,
        "target_reshuffle_interval_hours": 1
    },
    {
        "name": "S4: Smaller Net, Fewer Shards",
        "num_shards": 20,
        "max_plot_size_gib": 100,
        "total_network_storage_pib": 20,
        "plotting_throughput_tib_hr_per_attacker": 3.6,
        "attacker_storage_percentage": 0.15, # 15%
        "cost_per_plot_usd": 0.25,
        "target_reshuffle_interval_hours": 1
    },
    {
        "name": "S5: Fast Blocks, Larger Size",
        "num_shards": 50,
        "max_plot_size_gib": 100,
        "total_network_storage_pib": 50,
        "plotting_throughput_tib_hr_per_attacker": 3.6,
        "attacker_storage_percentage": 0.05,
        "cost_per_plot_usd": 0.25,
        "target_reshuffle_interval_hours": 1,
        "avg_block_time_seconds": 1, # Faster block time
        "avg_block_size_mb": 2 # Larger block size
    },
    {
        "name": "S6: Longer Reshuffle",
        "num_shards": 100,
        "max_plot_size_gib": 100,
        "total_network_storage_pib": 500,
        "plotting_throughput_tib_hr_per_attacker": 3.6,
        "attacker_storage_percentage": 0.01,
        "cost_per_plot_usd": 0.25,
        "target_reshuffle_interval_hours": 24 # 24 hours
    },
     {
        "name": "S7: High Attacker, More Shards",
        "num_shards": 200, # More shards for same total storage
        "max_plot_size_gib": 100,
        "total_network_storage_pib": 500,
        "plotting_throughput_tib_hr_per_attacker": 3.6,
        "attacker_storage_percentage": 0.15, # High attacker share
        "cost_per_plot_usd": 0.25,
        "target_reshuffle_interval_hours": 1
    },
    {
        "name": "S8: Very Low Attacker Plotting (Hypothetical)",
        "num_shards": 100,
        "max_plot_size_gib": 100,
        "total_network_storage_pib": 500,
        "plotting_throughput_tib_hr_per_attacker": 0.1, # Very slow plotting speed
        "attacker_storage_percentage": 0.01,
        "cost_per_plot_usd": 0.25,
        "target_reshuffle_interval_hours": 1
    },
    {
        "name": "S9: High Plotting Cost",
        "num_shards": 100,
        "max_plot_size_gib": 100,
        "total_network_storage_pib": 500,
        "plotting_throughput_tib_hr_per_attacker": 3.6,
        "attacker_storage_percentage": 0.05,
        "cost_per_plot_usd": 1.50, # Significantly higher cost per plot
        "target_reshuffle_interval_hours": 1
    },
    # Starting here I am including a set of test scenarios to see if the model makes sense and the computation 
    # of probabilities is correct.
   {
        "name": "STEST1: High Attacker % - Low Shards (DEMO NON-ZERO PROB)",
        "num_shards": 2, # Very few shards
        "max_plot_size_gib": 100,
        "total_network_storage_pib": 100, # Smaller network size
        "plotting_throughput_tib_hr_per_attacker": 3.6,
        "attacker_storage_percentage": 0.45, # Attacker controls 45% of total network
        "cost_per_plot_usd": 0.25,
        "target_reshuffle_interval_hours": 1
    },
  {
        "name": "STEST2: Moderate Prob. Demo 0-1 prob.", # NEW SCENARIO
        "num_shards": 5, # Few shards
        "max_plot_size_gib": 100,
        "total_network_storage_pib": 1, # Small total network size (1 PiB)
        "plotting_throughput_tib_hr_per_attacker": 3.6,
        "attacker_storage_percentage": 0.50, # Attacker controls 50% of total network
        "cost_per_plot_usd": 0.25,
        "target_reshuffle_interval_hours": 1
    },
    # TODO: Placeholder to discuss this sync with Nazar.
   {
        "name": "STEST1: High Attacker % - Low Shards (DEMO NON-ZERO PROB)",
        "num_shards": 2,
        "max_plot_size_gib": 100,
        "total_network_storage_pib": 100,
        "plotting_throughput_tib_hr_per_attacker": 3.6,
        "attacker_storage_percentage": 0.45,
        "cost_per_plot_usd": 0.25,
        "target_reshuffle_interval_hours": 1
    },
]

# --- Execute and Display ---
def main():
    table = PrettyTable()
    table.field_names = [
        "Scenario", "N (Shards)", "Net Storage (PiB)", "Plot Size (GiB)", "Attacker %",
        "k_alpha (plots)", "K_M (plots)", "Plotting Prob.", "Plot Time (days)",
        "Economic Cost (USD)", # NEW FIELD
        "Reshuffle (hrs)", "Sync Data (GB/hr)", "Plot Attack Risk", "Alloc. Prob. Risk", "Sync Feasibility"
    ]
    table.align = "l" # Align all columns to the left

    for scenario_params in scenarios:
        scenario_name = scenario_params.pop("name") # Extract 'name' before passing to evaluate_scenario
        row_data = evaluate_scenario(**scenario_params)
        
        # Manually add scenario name to the beginning of row_data
        row_list = [row_data[field] for field in table.field_names[1:]] # Start from index 1 as 'Scenario' is handled manually
        row_list.insert(0, scenario_name) # Insert scenario_name at the beginning
        table.add_row(row_list)

    print("--- Protocol Parameter Evaluation Summary ---")
    print("Metrics are calculated for achieving 51% control of a single shard.")
    print("Plotting Prob.: Probability of an attacker getting k_alpha plots in a specific shard.")
    print("Plot Time (days): Days for attacker to generate all their K_M plots.")
    print("Economic Cost (USD): Estimated total cost for an attacker to acquire K_M plots.")
    print("Sync Data (GB/hr): Data a farmer needs to download to sync 1 hour of blocks from new shard.")
    print("\n" + str(table))
    print("\n**Risk Assessment Legend:**")
    print("  Plot Attack Risk:")
    print("    - Low: Attacker cannot plot fast enough to respond to reshuffles.")
    print("    - Medium: Attacker could potentially plot enough within ~1 day.")
    print("    - High: Attacker can plot enough within the reshuffle interval or less.")
    print("  Alloc. Prob. Risk:")
    print("    - Very Low: Probability of random allocation resulting in shard control is negligible.")
    print("    - Moderate: Low but non-negligible probability (e.g., >0.0001).")
    print("    - High: Significant probability (e.g., >0.01).")
    print("  Sync Feasibility:")
    print("    - Manageable: Data sync is low, generally fine for most users.")
    print("    - Demanding: Data sync is higher, may strain some connections or require optimized clients.")
    print("    - Very Demanding: High data sync, likely problematic for many users.")


if __name__ == "__main__":
    main()