use ab_data_retrieval::piece_getter::PieceGetter;
use ab_erasure_coding::ErasureCoding;
use ab_farmer::plotter::gpu::GpuPlotter;
use ab_proof_of_space_gpu::Device;
use async_lock::{Mutex as AsyncMutex, Semaphore};
use clap::Parser;
use prometheus_client::registry::Registry;
use std::collections::BTreeSet;
use std::num::{NonZeroU8, NonZeroUsize};
use std::sync::Arc;
use tracing::{debug, info, warn};

#[derive(Debug, Parser)]
pub(in super::super) struct GpuPlottingOptions {
    /// How many records the farmer will encode concurrently on the same GPU.
    ///
    /// Increasing this value will cause higher VRAM usage and will not necessarily improve
    /// performance.
    ///
    /// Defaults to 4 for dGPU and 2 otherwise (iGPU, etc.).
    #[arg(long)]
    gpu_record_encoding_concurrency: Option<NonZeroU8>,
    /// How many sectors a farmer will download concurrently during plotting with GPUs.
    ///
    /// Limits memory usage of the plotting process. Defaults to the number of GPUs * 3,
    /// to download future sectors ahead of time.
    ///
    /// Increasing this value will cause higher memory usage.
    #[arg(long)]
    gpu_sector_downloading_concurrency: Option<NonZeroUsize>,
    /// Set the exact GPUs to be used for plotting instead of using recommended GPUs (default
    /// behavior).
    ///
    /// By default, dGPUs are used if available, if not, then iGPUs are used, if neither dGPU nor
    /// iGPU is found, virtual GPU will be used as the last resort.
    ///
    /// GPUs are coma-separated: `--gpus 0,1,3`. Use an empty string to disable GPU plotting.
    #[arg(long)]
    gpus: Option<String>,
}

pub(in super::super) async fn init_gpu_plotter<PG>(
    gpu_plotting_options: GpuPlottingOptions,
    piece_getter: PG,
    global_mutex: Arc<AsyncMutex<()>>,
    erasure_coding: ErasureCoding,
    registry: &mut Registry,
) -> anyhow::Result<Option<GpuPlotter<PG>>>
where
    PG: PieceGetter + Clone + Send + Sync + 'static,
{
    let GpuPlottingOptions {
        gpu_record_encoding_concurrency: _,
        gpu_sector_downloading_concurrency,
        gpus,
    } = gpu_plotting_options;

    let all_gpu_devices = vec![Device];

    let used_gpu_devices = if let Some(gpus) = gpus {
        if gpus.is_empty() {
            info!("GPU plotting was explicitly disabled");
            return Ok(None);
        }

        let mut gpus_to_use = gpus
            .split(',')
            .map(str::parse)
            .collect::<Result<BTreeSet<u32>, _>>()?;

        let gpu_devices = all_gpu_devices
            .into_iter()
            .filter(|_device| gpus_to_use.remove(&0))
            .collect::<Vec<_>>();

        if !gpus_to_use.is_empty() {
            warn!(?gpus_to_use, "Some GPUs were not found on the system");
        }

        gpu_devices
    } else {
        all_gpu_devices
    };

    if used_gpu_devices.is_empty() {
        debug!("No GPU devices were found or used");
        return Ok(None);
    }

    let downloading_semaphore = Arc::new(Semaphore::new(
        gpu_sector_downloading_concurrency.map_or(used_gpu_devices.len() * 3, NonZeroUsize::get),
    ));

    Ok(Some(
        GpuPlotter::new(
            piece_getter,
            downloading_semaphore,
            used_gpu_devices
                .into_iter()
                .map(|device| device.instantiate())
                .collect(),
            global_mutex,
            erasure_coding,
            Some(registry),
        )
        .map_err(|error| anyhow::anyhow!("Failed to initialize GPU plotter: {error}"))?,
    ))
}
