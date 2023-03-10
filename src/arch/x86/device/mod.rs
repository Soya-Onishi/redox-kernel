pub mod cpu;
pub mod ioapic;
pub mod local_apic;
pub mod pic;
pub mod pit;
pub mod rtc;
pub mod serial;
#[cfg(feature = "acpi")]
pub mod hpet;
#[cfg(feature = "system76_ec_debug")]
pub mod system76_ec;

use crate::paging::KernelMapper;

pub unsafe fn init() {
    pic::init();
    local_apic::init(&mut KernelMapper::lock());
}
pub unsafe fn init_after_acpi()  {
    // this will disable the IOAPIC if needed.
    //ioapic::init(mapper);
}

#[cfg(feature = "acpi")]
unsafe fn init_hpet() -> bool {
    use crate::acpi::ACPI_TABLE;
    if let Some(ref mut hpet) = *ACPI_TABLE.hpet.write() {
        hpet::init(hpet)
    } else {
        false
    }
}

#[cfg(not(feature = "acpi"))]
unsafe fn init_hpet() -> bool {
    false
}

pub unsafe fn init_noncore() {
    if false /*TODO: init_hpet()*/ {
        log::info!("HPET used as system timer");
    } else {
        pit::init();
        log::info!("PIT used as system timer");
    }

    rtc::init();
    serial::init();
}

pub unsafe fn init_ap() {
    local_apic::init_ap();
}
