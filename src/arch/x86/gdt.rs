//! Global descriptor table

use core::convert::TryInto;
use core::mem;

use x86::segmentation::load_cs;
use x86::bits32::task::TaskStateSegment;
use x86::Ring;
use x86::dtables::{self, DescriptorTablePointer};
use x86::segmentation::{self, Descriptor as SegmentDescriptor, SegmentSelector};
use x86::task;

use super::cpuid::cpuid;

pub const GDT_NULL: usize = 0;
pub const GDT_KERNEL_CODE: usize = 1;
pub const GDT_KERNEL_DATA: usize = 2;
pub const GDT_KERNEL_KPCR: usize = 3;
pub const GDT_USER_CODE: usize = 4;
pub const GDT_USER_DATA: usize = 5;
pub const GDT_USER_FS: usize = 6;
pub const GDT_USER_GS: usize = 7;
pub const GDT_TSS: usize = 8;
pub const GDT_CPU_ID_CONTAINER: usize = 9;

pub const GDT_A_PRESENT: u8 = 1 << 7;
pub const GDT_A_RING_0: u8 = 0 << 5;
pub const GDT_A_RING_1: u8 = 1 << 5;
pub const GDT_A_RING_2: u8 = 2 << 5;
pub const GDT_A_RING_3: u8 = 3 << 5;
pub const GDT_A_SYSTEM: u8 = 1 << 4;
pub const GDT_A_EXECUTABLE: u8 = 1 << 3;
pub const GDT_A_CONFORMING: u8 = 1 << 2;
pub const GDT_A_PRIVILEGE: u8 = 1 << 1;
pub const GDT_A_DIRTY: u8 = 1;

pub const GDT_A_TSS_AVAIL: u8 = 0x9;
pub const GDT_A_TSS_BUSY: u8 = 0xB;

pub const GDT_F_PAGE_SIZE: u8 = 1 << 7;
pub const GDT_F_PROTECTED_MODE: u8 = 1 << 6;
pub const GDT_F_LONG_MODE: u8 = 1 << 5;

static mut INIT_GDT: [GdtEntry; 4] = [
    // Null
    GdtEntry::new(0, 0, 0, 0),
    // Kernel code
    GdtEntry::new(0, 0xFFFFF, GDT_A_PRESENT | GDT_A_RING_0 | GDT_A_SYSTEM | GDT_A_EXECUTABLE | GDT_A_PRIVILEGE, GDT_F_PAGE_SIZE | GDT_F_PROTECTED_MODE),
    // Kernel data
    GdtEntry::new(0, 0xFFFFF, GDT_A_PRESENT | GDT_A_RING_0 | GDT_A_SYSTEM | GDT_A_PRIVILEGE, GDT_F_PAGE_SIZE | GDT_F_PROTECTED_MODE),
    // Kernel TLS
    GdtEntry::new(0, 0xFFFFF, GDT_A_PRESENT | GDT_A_RING_0 | GDT_A_SYSTEM | GDT_A_PRIVILEGE, GDT_F_PAGE_SIZE | GDT_F_PROTECTED_MODE),
];

#[thread_local]
pub static mut GDT: [GdtEntry; 10] = [
    // Null
    GdtEntry::new(0, 0, 0, 0),
    // Kernel code
    GdtEntry::new(0, 0xFFFFF, GDT_A_PRESENT | GDT_A_RING_0 | GDT_A_SYSTEM | GDT_A_EXECUTABLE | GDT_A_PRIVILEGE, GDT_F_PAGE_SIZE | GDT_F_PROTECTED_MODE),
    // Kernel data
    GdtEntry::new(0, 0xFFFFF, GDT_A_PRESENT | GDT_A_RING_0 | GDT_A_SYSTEM | GDT_A_PRIVILEGE, GDT_F_PAGE_SIZE | GDT_F_PROTECTED_MODE),
    // Kernel TLS
    GdtEntry::new(0, 0xFFFFF, GDT_A_PRESENT | GDT_A_RING_0 | GDT_A_SYSTEM | GDT_A_PRIVILEGE, GDT_F_PAGE_SIZE | GDT_F_PROTECTED_MODE),
    // User (32-bit) code
    GdtEntry::new(0, 0xFFFFF, GDT_A_PRESENT | GDT_A_RING_3 | GDT_A_SYSTEM | GDT_A_EXECUTABLE | GDT_A_PRIVILEGE, GDT_F_PAGE_SIZE | GDT_F_PROTECTED_MODE),
    // User data
    GdtEntry::new(0, 0xFFFFF, GDT_A_PRESENT | GDT_A_RING_3 | GDT_A_SYSTEM | GDT_A_PRIVILEGE, GDT_F_PAGE_SIZE | GDT_F_PROTECTED_MODE),
    // User FS (for TLS)
    GdtEntry::new(0, 0xFFFFF, GDT_A_PRESENT | GDT_A_RING_3 | GDT_A_SYSTEM | GDT_A_PRIVILEGE, GDT_F_PAGE_SIZE | GDT_F_PROTECTED_MODE),
    // User GS (for TLS)
    GdtEntry::new(0, 0xFFFFF, GDT_A_PRESENT | GDT_A_RING_3 | GDT_A_SYSTEM | GDT_A_PRIVILEGE, GDT_F_PAGE_SIZE | GDT_F_PROTECTED_MODE),
    // TSS
    GdtEntry::new(0, 0, GDT_A_PRESENT | GDT_A_RING_3 | GDT_A_TSS_AVAIL, 0),
    // Unused entry which stores the CPU ID. This is necessary for paranoid interrupts as they have
    // no other way of determining it.
    GdtEntry::new(0, 0, 0, 0),
];

#[repr(C, align(16))]
pub struct ProcessorControlRegion {
    pub tcb_end: usize,
    pub user_rsp_tmp: usize,
    pub tss: TssWrapper,
}

// NOTE: Despite not using #[repr(packed)], we do know that while there may be some padding
// inserted before and after the TSS, the main TSS structure will remain intact.
#[repr(C, align(16))]
pub struct TssWrapper(pub TaskStateSegment);

#[thread_local]
pub static mut KPCR: ProcessorControlRegion = ProcessorControlRegion {
    tcb_end: 0,
    user_rsp_tmp: 0,
    tss: TssWrapper(TaskStateSegment::new()),
};

#[cfg(feature = "pti")]
pub unsafe fn set_tss_stack(stack: usize) {
    use super::pti::{PTI_CPU_STACK, PTI_CONTEXT_STACK};
    KPCR.tss.0.ss0 = (GDT_KERNEL_DATA << 3) as u16;
    KPCR.tss.0.esp0 = (PTI_CPU_STACK.as_ptr() as usize + PTI_CPU_STACK.len()) as u32;
    PTI_CONTEXT_STACK = stack;
}

#[cfg(not(feature = "pti"))]
pub unsafe fn set_tss_stack(stack: usize) {
    KPCR.tss.0.ss0 = (GDT_KERNEL_DATA << 3) as u16;
    KPCR.tss.0.esp0 = stack as u32;
}

// Initialize GDT
pub unsafe fn init() {
    {
        // Setup the initial GDT with TLS, so we can setup the TLS GDT (a little confusing)
        // This means that each CPU will have its own GDT, but we only need to define it once as a thread local

        let limit = (INIT_GDT.len() * mem::size_of::<GdtEntry>() - 1)
            .try_into()
            .expect("initial GDT way too large");
        let base = INIT_GDT.as_ptr() as *const SegmentDescriptor;

        let init_gdtr: DescriptorTablePointer<SegmentDescriptor> = DescriptorTablePointer {
            limit,
            base,
        };

        // Load the initial GDT, before we have access to thread locals
        dtables::lgdt(&init_gdtr);
    }

    // Load the segment descriptors
    load_cs(SegmentSelector::new(GDT_KERNEL_CODE as u16, Ring::Ring0));
    segmentation::load_ds(SegmentSelector::new(GDT_KERNEL_DATA as u16, Ring::Ring0));
    segmentation::load_es(SegmentSelector::new(GDT_KERNEL_DATA as u16, Ring::Ring0));
    segmentation::load_fs(SegmentSelector::new(GDT_KERNEL_DATA as u16, Ring::Ring0));
    segmentation::load_gs(SegmentSelector::new(GDT_KERNEL_KPCR as u16, Ring::Ring0));
    segmentation::load_ss(SegmentSelector::new(GDT_KERNEL_DATA as u16, Ring::Ring0));
}

/// Initialize GDT with TLS
pub unsafe fn init_paging(cpu_id: u32, tcb_offset: usize, stack_offset: usize) {
    //TODO: will this work with multicore?
    {
        INIT_GDT[GDT_KERNEL_KPCR].set_offset(tcb_offset as u32);
        segmentation::load_gs(SegmentSelector::new(GDT_KERNEL_KPCR as u16, Ring::Ring0));
    }

    // Now that we have access to thread locals, begin by getting a pointer to the Processor
    // Control Region.
    let kpcr = &mut KPCR;

    // Then, setup the AP's individual GDT
    let limit = (GDT.len() * mem::size_of::<GdtEntry>() - 1)
        .try_into()
        .expect("main GDT way too large");
    let base = GDT.as_ptr() as *const SegmentDescriptor;

    let gdtr: DescriptorTablePointer<SegmentDescriptor> = DescriptorTablePointer {
        limit,
        base,
    };

    // Once we have fetched the real KPCR address, set the TLS segment to the TCB pointer there.
    kpcr.tcb_end = (tcb_offset as *const usize).read();

    GDT[GDT_KERNEL_KPCR].set_offset(tcb_offset as u32);

    {
        // We can now access our TSS, via the KPCR, which is a thread local
        let tss = &kpcr.tss.0 as *const _ as usize as u32;

        GDT[GDT_TSS].set_offset(tss);
        GDT[GDT_TSS].set_limit(mem::size_of::<TaskStateSegment>() as u32);
    }

    // And finally, populate the last GDT entry with the current CPU ID, to allow paranoid
    // interrupt handlers to safely use TLS.
    (&mut GDT[GDT_CPU_ID_CONTAINER] as *mut GdtEntry).cast::<u32>().write(cpu_id);

    // Set the stack pointer to use when coming back from userspace.
    set_tss_stack(stack_offset);

    // Load the new GDT, which is correctly located in thread local storage.
    dtables::lgdt(&gdtr);

    // Reload the segment descriptors
    load_cs(SegmentSelector::new(GDT_KERNEL_CODE as u16, Ring::Ring0));
    segmentation::load_ds(SegmentSelector::new(GDT_KERNEL_DATA as u16, Ring::Ring0));
    segmentation::load_es(SegmentSelector::new(GDT_KERNEL_DATA as u16, Ring::Ring0));
    segmentation::load_fs(SegmentSelector::new(GDT_KERNEL_DATA as u16, Ring::Ring0));
    segmentation::load_gs(SegmentSelector::new(GDT_KERNEL_KPCR as u16, Ring::Ring0));
    segmentation::load_ss(SegmentSelector::new(GDT_KERNEL_DATA as u16, Ring::Ring0));

    // Load the task register
    task::load_tr(SegmentSelector::new(GDT_TSS as u16, Ring::Ring0));
}

#[derive(Copy, Clone, Debug)]
#[repr(packed)]
pub struct GdtEntry {
    pub limitl: u16,
    pub offsetl: u16,
    pub offsetm: u8,
    pub access: u8,
    pub flags_limith: u8,
    pub offseth: u8
}

impl GdtEntry {
    pub const fn new(offset: u32, limit: u32, access: u8, flags: u8) -> Self {
        GdtEntry {
            limitl: limit as u16,
            offsetl: offset as u16,
            offsetm: (offset >> 16) as u8,
            access,
            flags_limith: flags & 0xF0 | ((limit >> 16) as u8) & 0x0F,
            offseth: (offset >> 24) as u8
        }
    }

    pub fn offset(&self) -> u32 {
        (self.offsetl as u32) |
        ((self.offsetm as u32) << 16) |
        ((self.offseth as u32) << 24)
    }

    pub fn set_offset(&mut self, offset: u32) {
        self.offsetl = offset as u16;
        self.offsetm = (offset >> 16) as u8;
        self.offseth = (offset >> 24) as u8;
    }

    pub fn set_limit(&mut self, limit: u32) {
        self.limitl = limit as u16;
        self.flags_limith = self.flags_limith & 0xF0 | ((limit >> 16) as u8) & 0x0F;
    }
}
