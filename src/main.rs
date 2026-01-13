use serde::Deserialize;
use std::fs;
use std::thread::sleep;
use std::time::Duration;
use windows::Win32::Foundation::{CloseHandle, BOOL};
use windows::Win32::System::Threading::{
    GetPriorityClass, GetProcessAffinityMask, GetProcessInformation, OpenProcess, SetPriorityClass,
    SetProcessAffinityMask, SetProcessInformation, ABOVE_NORMAL_PRIORITY_CLASS,
    BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS, MEMORY_PRIORITY,
    MEMORY_PRIORITY_INFORMATION, NORMAL_PRIORITY_CLASS, PROCESS_ACCESS_RIGHTS,
    PROCESS_CREATION_FLAGS, PROCESS_INFORMATION_CLASS, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
    PROCESS_QUERY_INFORMATION, PROCESS_SET_INFORMATION, REALTIME_PRIORITY_CLASS,
};

#[allow(non_camel_case_types)]
#[repr(transparent)]
struct IO_PRIORITY_HINT(pub i32);

use windows::Win32::System::ProcessStatus::{EnumProcesses, GetProcessImageFileNameW};

#[derive(Deserialize)]
struct Config {
    settings: Settings,
    targets: Vec<Target>,
}

#[derive(Deserialize)]
struct Settings {
    refresh_interval_secs: u64,
}

#[derive(Deserialize)]
struct Target {
    name: String,
    priority: String,
    affinity: Vec<u32>,
    io_priority: Option<String>,
    memory_priority: Option<String>,
    power_throttling: Option<String>,
}

fn get_priority_class(prio_str: &str) -> u32 {
    match prio_str.to_lowercase().as_str() {
        "idle" => IDLE_PRIORITY_CLASS.0,
        "below_normal" => BELOW_NORMAL_PRIORITY_CLASS.0,
        "normal" => NORMAL_PRIORITY_CLASS.0,
        "above_normal" => ABOVE_NORMAL_PRIORITY_CLASS.0,
        "high" => HIGH_PRIORITY_CLASS.0,
        "realtime" => REALTIME_PRIORITY_CLASS.0,
        _ => NORMAL_PRIORITY_CLASS.0,
    }
}

fn get_io_priority_hint(s: &str) -> Option<IO_PRIORITY_HINT> {
    match s.to_lowercase().as_str() {
        "very_low" => Some(IO_PRIORITY_HINT(0)),
        "low" => Some(IO_PRIORITY_HINT(1)),
        "normal" => Some(IO_PRIORITY_HINT(2)),
        "high" => Some(IO_PRIORITY_HINT(3)),
        "critical" => Some(IO_PRIORITY_HINT(4)),
        _ => None,
    }
}

fn get_memory_priority(s: &str) -> Option<u32> {
    match s.to_lowercase().as_str() {
        "lowest" => Some(0),
        "very_low" => Some(1),
        "low" => Some(2),
        "medium" => Some(3),
        "below_normal" => Some(4),
        "normal" => Some(5),
        _ => None,
    }
}

fn main() {
    let config_str = fs::read_to_string("config.toml").expect("Failed to read config.toml");
    let config: Config = toml::from_str(&config_str).expect("Failed to parse config.toml");

    println!(
        "Loaded {} targets. Starting infinite monitor...",
        config.targets.len()
    );
    println!("Press Ctrl+C to stop. Edit config.toml to add more targets.");

    loop {
        let mut cb_needed: u32 = 0;
        let mut pids_buffer: Vec<u32> = vec![0u32; 1024];
        let cb_size = (pids_buffer.len() * std::mem::size_of::<u32>()) as u32;
        let enum_success =
            unsafe { EnumProcesses(pids_buffer.as_mut_ptr(), cb_size, &mut cb_needed) }.is_ok();
        if enum_success {
            let num_processes = (cb_needed / std::mem::size_of::<u32>() as u32) as usize;
            pids_buffer.truncate(num_processes);
            for &pid in &pids_buffer {
                if pid == 0 {
                    continue;
                }
                let desired_access =
                    PROCESS_QUERY_INFORMATION.0 | PROCESS_SET_INFORMATION.0 | 0x1000u32;
                let h_query_result =
                    unsafe { OpenProcess(PROCESS_ACCESS_RIGHTS(desired_access), BOOL(0), pid) };
                if let Ok(h_query) = h_query_result {
                    let mut image_name = [0u16; 260];
                    let name_len = unsafe { GetProcessImageFileNameW(h_query, &mut image_name) };
                    let _ = unsafe { CloseHandle(h_query) };
                    if name_len > 0 {
                        let full_path = String::from_utf16_lossy(&image_name[0..name_len as usize]);
                        let pname_str = std::path::Path::new(&full_path)
                            .file_name()
                            .and_then(|os_str| os_str.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        let pname = pname_str.clone();
                        for target in &config.targets {
                            if pname == target.name.to_lowercase() {
                                let h_set_result = unsafe {
                                    OpenProcess(PROCESS_ACCESS_RIGHTS(desired_access), BOOL(0), pid)
                                };
                                if let Ok(handle) = h_set_result {
                                    // 1. Priority Class
                                    let current_prio = unsafe { GetPriorityClass(handle) };
                                    let target_prio = get_priority_class(&target.priority);
                                    if current_prio != target_prio {
                                        unsafe {
                                            let _ = SetPriorityClass(
                                                handle,
                                                PROCESS_CREATION_FLAGS(target_prio),
                                            );
                                        };
                                        println!(
                                            "Set Priority to {} for '{}' (PID {})",
                                            target.priority, pname_str, pid
                                        );
                                    }

                                    // 2. Affinity
                                    let mut process_affinity: usize = 0;
                                    let mut system_affinity: usize = 0;
                                    if unsafe {
                                        GetProcessAffinityMask(
                                            handle,
                                            &mut process_affinity,
                                            &mut system_affinity,
                                        )
                                    }
                                    .is_ok()
                                    {
                                        let mut target_mask: usize = 0;
                                        for &core in &target.affinity {
                                            if core < 64 {
                                                target_mask |= 1usize << core as usize;
                                            }
                                        }
                                        if process_affinity != target_mask {
                                            unsafe {
                                                let _ = SetProcessAffinityMask(handle, target_mask);
                                            };
                                            println!(
                                                "Set Affinity to 0x{:x} for '{}' (PID {})",
                                                target_mask, pname_str, pid
                                            );
                                        }
                                    }

                                    // 3. IO Priority
                                    if let Some(io_prio_str) = &target.io_priority {
                                        if let Some(target_io_prio) =
                                            get_io_priority_hint(io_prio_str)
                                        {
                                            let mut current_io_prio = IO_PRIORITY_HINT(0);
                                            let ret = unsafe {
                                                GetProcessInformation(
                                                    handle,
                                                    PROCESS_INFORMATION_CLASS(21), // ProcessIoPriority
                                                    &mut current_io_prio as *mut _ as *mut _,
                                                    std::mem::size_of::<IO_PRIORITY_HINT>() as u32,
                                                )
                                            };
                                            if ret.is_ok() && current_io_prio.0 != target_io_prio.0
                                            {
                                                unsafe {
                                                    let _ = SetProcessInformation(
                                                        handle,
                                                        PROCESS_INFORMATION_CLASS(21), // ProcessIoPriority
                                                        &target_io_prio as *const _ as *const _,
                                                        std::mem::size_of::<IO_PRIORITY_HINT>()
                                                            as u32,
                                                    );
                                                }
                                                println!(
                                                    "Set IO Priority to {} for '{}' (PID {})",
                                                    io_prio_str, pname_str, pid
                                                );
                                            }
                                        }
                                    }

                                    // 4. Memory Priority
                                    if let Some(mem_prio_str) = &target.memory_priority {
                                        if let Some(target_mem_prio_val) =
                                            get_memory_priority(mem_prio_str)
                                        {
                                            let mut current_mem_prio =
                                                MEMORY_PRIORITY_INFORMATION {
                                                    MemoryPriority: MEMORY_PRIORITY(0),
                                                };
                                            let ret = unsafe {
                                                GetProcessInformation(
                                                    handle,
                                                    PROCESS_INFORMATION_CLASS(39), // ProcessMemoryPriority
                                                    &mut current_mem_prio as *mut _ as *mut _,
                                                    std::mem::size_of::<MEMORY_PRIORITY_INFORMATION>(
                                                    )
                                                        as u32,
                                                )
                                            };
                                            if ret.is_ok()
                                                && current_mem_prio.MemoryPriority.0
                                                    != target_mem_prio_val
                                            {
                                                let target_mem_prio = MEMORY_PRIORITY_INFORMATION {
                                                    MemoryPriority: MEMORY_PRIORITY(
                                                        target_mem_prio_val,
                                                    ),
                                                };
                                                unsafe {
                                                    let _ = SetProcessInformation(
                                                        handle,
                                                        PROCESS_INFORMATION_CLASS(39), // ProcessMemoryPriority
                                                        &target_mem_prio as *const _ as *const _,
                                                        std::mem::size_of::<
                                                            MEMORY_PRIORITY_INFORMATION,
                                                        >(
                                                        )
                                                            as u32,
                                                    );
                                                }
                                                println!(
                                                    "Set Memory Priority to {} for '{}' (PID {})",
                                                    mem_prio_str, pname_str, pid
                                                );
                                            }
                                        }
                                    }

                                    // 5. Power Throttling
                                    if let Some(power_str) = &target.power_throttling {
                                        let enable = power_str.to_lowercase() == "enabled";
                                        let mut current_power = PROCESS_POWER_THROTTLING_STATE {
                                            Version: 0,
                                            ControlMask: 0,
                                            StateMask: 0,
                                        };
                                        let ret = unsafe {
                                            GetProcessInformation(
                                                handle,
                                                PROCESS_INFORMATION_CLASS(62), // ProcessPowerThrottling
                                                &mut current_power as *mut _ as *mut _,
                                                std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>(
                                                )
                                                    as u32,
                                            )
                                        };

                                        let target_state_mask = if enable {
                                            PROCESS_POWER_THROTTLING_EXECUTION_SPEED
                                        } else {
                                            0
                                        };
                                        let current_state_bit = current_power.StateMask
                                            & PROCESS_POWER_THROTTLING_EXECUTION_SPEED;

                                        if ret.is_ok() && current_state_bit != target_state_mask {
                                            let target_power = PROCESS_POWER_THROTTLING_STATE {
                                                Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                                                ControlMask:
                                                    PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
                                                StateMask: target_state_mask,
                                            };
                                            unsafe {
                                                let _ = SetProcessInformation(
                                                    handle,
                                                    PROCESS_INFORMATION_CLASS(62), // ProcessPowerThrottling
                                                    &target_power as *const _ as *const _,
                                                    std::mem::size_of::<
                                                        PROCESS_POWER_THROTTLING_STATE,
                                                    >(
                                                    )
                                                        as u32,
                                                );
                                            }
                                            println!(
                                                "Set Power Throttling to {} for '{}' (PID {})",
                                                power_str, pname_str, pid
                                            );
                                        }
                                    }

                                    unsafe {
                                        let _ = CloseHandle(handle);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        sleep(Duration::from_secs(config.settings.refresh_interval_secs));
    }
}
