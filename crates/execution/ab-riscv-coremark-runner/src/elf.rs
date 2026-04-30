use ab_riscv_interpreter::prelude::*;
use anyhow::Context;
use object::{Object, ObjectSection, ObjectSymbol};

pub(crate) struct LoadedElf<'a> {
    pub(crate) entry_point: u64,
    pub(crate) global_pointer: u64,
    pub(crate) text_addr: u64,
    pub(crate) text_data: &'a [u8],
    pub(crate) output_buf_addr: u64,
    pub(crate) output_buf_size: u32,
}

/// Load all non-empty ELF sections into guest memory
pub(crate) fn load_elf<'a, Memory>(
    elf: &'a [u8],
    memory: &mut Memory,
) -> anyhow::Result<LoadedElf<'a>>
where
    Memory: VirtualMemory,
{
    let obj = object::File::parse(elf).context("failed to parse Coremark ELF")?;

    let mut output_buf_addr: Option<u64> = None;
    let mut output_buf_size: Option<u32> = None;

    for section in obj.sections() {
        let addr = section.address();
        if addr == 0 {
            continue;
        }

        if section.name().ok() == Some(".output_buf") {
            output_buf_addr = Some(addr);
            output_buf_size = Some(
                u32::try_from(section.size()).context(".output_buf section larger than 4 GiB")?,
            );
        }

        let data = section.data().context("failed to read ELF section data")?;
        if data.is_empty() {
            continue;
        }
        memory
            .write_slice(addr, data)
            .map_err(|e| anyhow::anyhow!(e))
            .with_context(|| format!("section at {addr:#x} does not fit in guest memory"))?;
    }

    let output_buf_addr =
        output_buf_addr.context("ELF missing .output_buf section; check core_portme.c")?;
    let output_buf_size =
        output_buf_size.context("ELF missing .output_buf section; check core_portme.c")?;

    // Resolve entry point via the `main` symbol rather than e_entry. With -static-pie the ELF entry
    // point is a relocation-applying thunk that writes GOT fixups to low addresses before calling
    // main; jumping straight to main bypasses that stub, which is correct since the interpreter
    // loads sections verbatim and GOT entries are already at their final addresses.
    let entry_point = obj
        .symbols()
        .find(|s| s.name().ok() == Some("main"))
        .map(|s| s.address())
        .context("no `main` symbol found in Coremark ELF")?;

    let global_pointer = obj
        .symbols()
        .find(|s| s.name().ok() == Some("__global_pointer$"))
        .map(|s| s.address())
        .context("no __global_pointer$ symbol")?;

    let text_section = obj
        .section_by_name(".text")
        .context("no .text section in Coremark ELF")?;
    let text_addr = text_section.address();
    let text_data = text_section
        .data()
        .context("failed to read .text section data")?;

    Ok(LoadedElf {
        entry_point,
        global_pointer,
        text_addr,
        text_data,
        output_buf_addr,
        output_buf_size,
    })
}
