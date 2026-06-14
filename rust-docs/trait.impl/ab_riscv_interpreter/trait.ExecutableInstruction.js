(function() {
    const implementors = Object.fromEntries([["ab_contract_file",[["impl&lt;Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError&gt; ExecutableInstruction&lt;Regs, ExtState, Memory, PC, InstructionHandler, CustomError&gt; for <a class=\"enum\" href=\"ab_contract_file/instruction/enum.ContractInstruction.html\" title=\"enum ab_contract_file::instruction::ContractInstruction\">ContractInstruction</a>&lt;Reg&gt;<div class=\"where\">where\n    Reg: Register + Register&lt;Type = <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u64.html\">u64</a>&gt; + ZcmpRegister&lt;Type = <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u64.html\">u64</a>&gt;,\n    Regs: RegisterFile&lt;Reg&gt;,\n    Memory: VirtualMemory,\n    PC: ProgramCounter&lt;Reg::Type, Memory, CustomError&gt;,\n    InstructionHandler: SystemInstructionHandler&lt;Reg, Regs, Memory, PC, CustomError&gt;,</div>",0]]],["ab_riscv_interpreter",[]]]);
    if (window.register_implementors) {
        window.register_implementors(implementors);
    } else {
        window.pending_implementors = implementors;
    }
})()
//{"start":59,"fragment_lengths":[905,28]}