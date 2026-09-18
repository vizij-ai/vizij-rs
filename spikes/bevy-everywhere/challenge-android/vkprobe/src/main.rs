//! Prints, for every Vulkan physical device the loader exposes, the two
//! properties wgpu-hal 29 derives `max_buffer_size` from
//! (`VkPhysicalDeviceMaintenance3Properties::maxMemoryAllocationSize` and
//! `VkPhysicalDeviceMaintenance4Properties::maxBufferSize`), queried the way
//! wgpu-hal does (chained into `vkGetPhysicalDeviceProperties2` when the
//! device API version or the extension allows it), plus the extensions that
//! gate `SURFACE_VIEW_FORMATS` and the maintenance structs.
use ash::{khr, vk};
use std::ffi::CStr;

fn main() {
    let entry = unsafe { ash::Entry::load() }.expect("load libvulkan");
    let inst_version = unsafe { entry.try_enumerate_instance_version() }
        .ok()
        .flatten()
        .unwrap_or(vk::API_VERSION_1_0);
    println!(
        "instance apiVersion {}.{}.{}",
        vk::api_version_major(inst_version),
        vk::api_version_minor(inst_version),
        vk::api_version_patch(inst_version)
    );
    let inst_exts = unsafe { entry.enumerate_instance_extension_properties(None) }.unwrap_or_default();
    let has_gpdp2 = inst_exts.iter().any(|e| {
        (unsafe { CStr::from_ptr(e.extension_name.as_ptr()) }) == khr::get_physical_device_properties2::NAME
    });
    println!("instance ext VK_KHR_get_physical_device_properties2: {has_gpdp2}");
    let app = vk::ApplicationInfo::default().api_version(if inst_version >= vk::API_VERSION_1_1 {
        vk::API_VERSION_1_1
    } else {
        vk::API_VERSION_1_0
    });
    let mut ext_ptrs = Vec::new();
    if has_gpdp2 {
        ext_ptrs.push(khr::get_physical_device_properties2::NAME.as_ptr());
    }
    let info = vk::InstanceCreateInfo::default()
        .application_info(&app)
        .enabled_extension_names(&ext_ptrs);
    let instance = unsafe { entry.create_instance(&info, None) }.expect("create instance");
    let gpdp2 = khr::get_physical_device_properties2::Instance::new(&entry, &instance);
    let devices = unsafe { instance.enumerate_physical_devices() }.expect("enumerate");
    println!("physical devices: {}", devices.len());
    for phd in devices {
        let props = unsafe { instance.get_physical_device_properties(phd) };
        let name = unsafe { CStr::from_ptr(props.device_name.as_ptr()) }.to_string_lossy();
        let v = props.api_version;
        println!(
            "\ndevice {name:?} type {:?} vendor 0x{:x} device 0x{:x} driverVersion 0x{:x} apiVersion {}.{}.{}",
            props.device_type,
            props.vendor_id,
            props.device_id,
            props.driver_version,
            vk::api_version_major(v),
            vk::api_version_minor(v),
            vk::api_version_patch(v)
        );
        let exts = unsafe { instance.enumerate_device_extension_properties(phd) }.unwrap_or_default();
        let has = |n: &CStr| exts.iter().any(|e| (unsafe { CStr::from_ptr(e.extension_name.as_ptr()) }) == n);
        let m3_ext = has(khr::maintenance3::NAME);
        let m4_ext = has(khr::maintenance4::NAME);
        println!("  device extensions: {}", exts.len());
        println!("  VK_KHR_maintenance3: {m3_ext}  VK_KHR_maintenance4: {m4_ext}  VK_KHR_swapchain_mutable_format: {}  VK_KHR_swapchain: {}",
            has(khr::swapchain_mutable_format::NAME), has(khr::swapchain::NAME));
        // Mirror wgpu-hal-29.0.4/src/vulkan/adapter.rs:1764-1767, 1787-1799.
        let supports_m3 = v >= vk::API_VERSION_1_1 || m3_ext;
        let supports_m4 = v >= vk::API_VERSION_1_3 || m4_ext;
        println!("  wgpu-hal would chain: maintenance3 {supports_m3}, maintenance4 {supports_m4}");
        if !has_gpdp2 {
            println!("  (no properties2: wgpu-hal would leave both None -> u64::MAX)");
            continue;
        }
        let mut m3 = vk::PhysicalDeviceMaintenance3Properties::default();
        let mut m4 = vk::PhysicalDeviceMaintenance4Properties::default();
        let mut p2 = vk::PhysicalDeviceProperties2::default();
        if supports_m3 {
            p2 = p2.push_next(&mut m3);
        }
        if supports_m4 {
            p2 = p2.push_next(&mut m4);
        }
        unsafe { gpdp2.get_physical_device_properties2(phd, &mut p2) };
        let m3v = m3.max_memory_allocation_size;
        let m4v = m4.max_buffer_size;
        println!("  maintenance3.maxMemoryAllocationSize = {m3v} ({} MiB)", m3v / (1 << 20));
        println!("  maintenance3.maxPerSetDescriptors    = {}", m3.max_per_set_descriptors);
        println!("  maintenance4.maxBufferSize           = {m4v} ({} MiB)", m4v / (1 << 20));
        let mb = if supports_m4 { m4v } else { u64::MAX };
        let ma = if supports_m3 { m3v } else { u64::MAX };
        let cap = i32::MAX as u64; // android, non-nvidia
        println!("  -> wgpu-hal max_buffer_size = min({mb}, {ma}, {cap}) = {}", mb.min(ma).min(cap));
    }
    unsafe { instance.destroy_instance(None) };
}
