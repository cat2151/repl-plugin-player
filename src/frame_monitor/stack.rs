//! Call stacks of another thread of this process, read while that thread is suspended.
//!
//! While the thread is suspended nothing here allocates or takes a lock: it may hold the
//! heap lock or the loader lock. So the unwind tables come from a module list taken
//! before the suspension ([`Stacks::refresh`]), and names are looked up afterwards
//! ([`Stacks::describe`]).

use std::{
    ffi::OsString,
    os::windows::ffi::{OsStrExt, OsStringExt},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::{
            Debug::{
                GetThreadContext, RtlVirtualUnwind, SymFromAddrW, SymInitializeW,
                SymRefreshModuleList, SymSetOptions, CONTEXT, CONTEXT_FULL_AMD64,
                IMAGE_RUNTIME_FUNCTION_ENTRY, SYMBOL_INFOW, SYMFLAG_EXPORT, SYMOPT_UNDNAME,
                UNW_FLAG_NHANDLER,
            },
            ToolHelp::{
                CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W,
                TH32CS_SNAPMODULE,
            },
        },
        Threading::{
            GetCurrentProcess, GetCurrentProcessId, OpenThread, ResumeThread, SuspendThread,
            THREAD_GET_CONTEXT, THREAD_QUERY_INFORMATION, THREAD_SUSPEND_RESUME,
        },
    },
};

const DEPTH: usize = 48;

#[repr(C, align(16))]
struct AlignedContext(CONTEXT);

struct Module {
    base: u64,
    size: u64,
    name: String,
    functions: *const IMAGE_RUNTIME_FUNCTION_ENTRY,
    count: usize,
}

pub struct Stacks {
    thread: HANDLE,
    modules: Vec<Module>,
    symbols: bool,
}

// The handle and the module tables are only used by the thread that owns `Stacks`.
unsafe impl Send for Stacks {}

impl Stacks {
    pub fn open(thread_id: u32) -> Option<Self> {
        let access = THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION;
        let thread = unsafe { OpenThread(access, 0, thread_id) };
        (!thread.is_null()).then(|| Self {
            thread,
            modules: Vec::new(),
            symbols: false,
        })
    }

    /// Re-reads the loaded modules and their unwind tables.
    pub fn refresh(&mut self) {
        self.modules.clear();
        let snapshot =
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, GetCurrentProcessId()) };
        if snapshot == INVALID_HANDLE_VALUE {
            return;
        }
        let mut entry: MODULEENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
        let mut more = unsafe { Module32FirstW(snapshot, &mut entry) } != 0;
        while more {
            let base = entry.modBaseAddr as u64;
            let (functions, count) = unsafe { unwind_table(entry.modBaseAddr) };
            let end = entry.szModule.iter().position(|c| *c == 0).unwrap_or(0);
            self.modules.push(Module {
                base,
                size: entry.modBaseSize as u64,
                name: OsString::from_wide(&entry.szModule[..end])
                    .to_string_lossy()
                    .into_owned(),
                functions,
                count,
            });
            more = unsafe { Module32NextW(snapshot, &mut entry) } != 0;
        }
        self.modules.sort_by_key(|m| m.base);
        unsafe { CloseHandle(snapshot) };
    }

    /// The thread's return addresses, innermost first; empty when it could not be read.
    pub fn sample(&self) -> Vec<u64> {
        let mut frames = [0u64; DEPTH];
        let mut context: AlignedContext = unsafe { std::mem::zeroed() };
        context.0.ContextFlags = CONTEXT_FULL_AMD64;
        if unsafe { SuspendThread(self.thread) } == u32::MAX {
            return Vec::new();
        }
        let mut depth = 0;
        if unsafe { GetThreadContext(self.thread, &mut context.0) } != 0 {
            depth = unsafe { self.unwind(&mut context.0, &mut frames) };
        }
        unsafe { ResumeThread(self.thread) };
        frames[..depth].to_vec()
    }

    /// Runs with the thread suspended: no allocation, no lock.
    unsafe fn unwind(&self, context: &mut CONTEXT, frames: &mut [u64; DEPTH]) -> usize {
        let mut depth = 0;
        while depth < DEPTH && context.Rip != 0 {
            let pc = context.Rip;
            frames[depth] = pc;
            depth += 1;
            let Some(module) = self.module(pc) else { break };
            match function(module, pc) {
                Some(entry) => {
                    let mut handler = std::ptr::null_mut();
                    let mut frame = 0;
                    RtlVirtualUnwind(
                        UNW_FLAG_NHANDLER,
                        module.base,
                        pc,
                        entry,
                        context,
                        &mut handler,
                        &mut frame,
                        std::ptr::null_mut(),
                    );
                }
                // Only a leaf function has no unwind entry; its return address is on top.
                None if depth == 1 => {
                    context.Rip = *(context.Rsp as *const u64);
                    context.Rsp += 8;
                }
                None => break,
            }
        }
        depth
    }

    fn module(&self, pc: u64) -> Option<&Module> {
        let index = self
            .modules
            .partition_point(|m| m.base <= pc)
            .checked_sub(1)?;
        let module = &self.modules[index];
        (pc < module.base + module.size).then_some(module)
    }

    /// `module!symbol` from debug info, otherwise `module+0x..`.
    pub fn describe(&mut self, pc: u64) -> String {
        let process = unsafe { GetCurrentProcess() };
        if !self.symbols {
            unsafe {
                // Deferred loading finds no symbols in the Rust executable's PDB, and
                // the PDB is found only beside the executable.
                SymSetOptions(SYMOPT_UNDNAME);
                let search: Option<Vec<u16>> = std::env::current_exe().ok().and_then(|exe| {
                    let folder = exe.parent()?.as_os_str().encode_wide();
                    Some(folder.chain([0]).collect())
                });
                let search = search.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
                SymInitializeW(process, search, 1);
            }
            self.symbols = true;
        }
        let Some(module) = self.module(pc) else {
            return format!("{pc:#x}");
        };
        const NAME: usize = 512;
        #[repr(C, align(8))]
        struct Buffer(SYMBOL_INFOW, [u16; NAME]);
        let mut buffer: Buffer = unsafe { std::mem::zeroed() };
        buffer.0.SizeOfStruct = std::mem::size_of::<SYMBOL_INFOW>() as u32;
        buffer.0.MaxNameLen = NAME as u32;
        let mut displacement = 0;
        let found = unsafe { SymFromAddrW(process, pc, &mut displacement, &mut buffer.0) } != 0;
        // Export names of modules without debug info point at unrelated functions.
        if found && buffer.0.Flags & SYMFLAG_EXPORT == 0 {
            let name = unsafe {
                std::slice::from_raw_parts(buffer.0.Name.as_ptr(), buffer.0.NameLen as usize)
            };
            let name = OsString::from_wide(name).to_string_lossy().into_owned();
            return format!("{}!{name}", module.name);
        }
        format!("{}+{:#x}", module.name, pc - module.base)
    }

    /// Lets [`describe`](Self::describe) see modules loaded since the last call.
    pub fn refresh_symbols(&self) {
        if self.symbols {
            unsafe { SymRefreshModuleList(GetCurrentProcess()) };
        }
    }
}

impl Drop for Stacks {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.thread) };
    }
}

/// The exception directory of a mapped PE32+ image.
unsafe fn unwind_table(base: *const u8) -> (*const IMAGE_RUNTIME_FUNCTION_ENTRY, usize) {
    let none = (std::ptr::null(), 0);
    if base.is_null() || *(base as *const u16) != 0x5a4d {
        return none;
    }
    let nt = base.add(*(base.add(0x3c) as *const u32) as usize);
    // "PE\0\0", then the 20-byte file header, then the PE32+ optional header.
    if *(nt as *const u32) != 0x4550 || *(nt.add(24) as *const u16) != 0x20b {
        return none;
    }
    let directories = *(nt.add(24 + 108) as *const u32);
    if directories <= 3 {
        return none;
    }
    let exception = nt.add(24 + 112 + 3 * 8);
    let (rva, size) = (
        *(exception as *const u32),
        *(exception.add(4) as *const u32),
    );
    if rva == 0 || size == 0 {
        return none;
    }
    (
        base.add(rva as usize) as *const IMAGE_RUNTIME_FUNCTION_ENTRY,
        size as usize / std::mem::size_of::<IMAGE_RUNTIME_FUNCTION_ENTRY>(),
    )
}

/// The unwind entry covering `pc`; the table is sorted by start address.
unsafe fn function(module: &Module, pc: u64) -> Option<*const IMAGE_RUNTIME_FUNCTION_ENTRY> {
    let rva = (pc - module.base) as u32;
    let (mut low, mut high) = (0, module.count);
    while low < high {
        let middle = (low + high) / 2;
        let entry = module.functions.add(middle);
        if rva < (*entry).BeginAddress {
            high = middle;
        } else if rva >= (*entry).EndAddress {
            low = middle + 1;
        } else {
            return Some(entry);
        }
    }
    None
}
