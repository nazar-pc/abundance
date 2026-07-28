#![expect(clippy::allow_attributes_without_reason)]
#![allow(clippy::let_underscore_untyped)]

use ab_core_primitives::pieces::Record;
use ab_core_primitives::sectors::SectorId;
use ab_farmer_components::plotting::RecordsEncoder;
use ab_farmer_components::sector::SectorContentsMap;
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::sync::atomic::AtomicBool;

/// Wrapper data structure encapsulating a single compatible device
#[derive(Clone, Debug)]
pub struct Device;

impl Device {
    pub fn instantiate(&self) -> GpuRecordsEncoder {
        let thread_pool = ThreadPoolBuilder::new().build().unwrap();

        GpuRecordsEncoder {
            instances: Vec::new(),
            thread_pool,
        }
    }
}

#[derive(Debug)]
pub struct GpuRecordsEncoder {
    instances: Vec<GpuRecordsEncoderInstance>,
    thread_pool: ThreadPool,
}

impl RecordsEncoder for GpuRecordsEncoder {
    fn encode_records(
        &mut self,
        _sector_id: &SectorId,
        _records: &mut [Record],
        _abort_early: &AtomicBool,
    ) -> anyhow::Result<SectorContentsMap> {
        self.thread_pool.install(|| {
            let _ = &self.instances[0];
        });

        Ok(SectorContentsMap::new(0))
    }
}

#[derive(Debug)]
struct GpuRecordsEncoderInstance {
    _device: wgpu::Device,
}
