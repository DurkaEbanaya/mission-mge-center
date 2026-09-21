/* about_system_dialog.rs
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use std::cell::RefCell;
use std::fs;

use adw::{prelude::*, subclass::prelude::*};
use gtk::{gio, glib};
use magpie_types::about::{about::OsInfo, About};

use crate::magpie_client::{DiskKind, HardwareInfo};
use crate::table_view::cached_icon::CachedIcon;
use crate::wayland_blur::WaylandBlur;
use crate::{to_human_readable_nice, DataType};

#[derive(Clone)]
struct InfoEntry {
    name: String,
    value: String,
}

struct InfoSection {
    title: String,
    entries: Vec<InfoEntry>,
}

fn clean(value: Option<String>) -> Option<String> {
    value.filter(|value| {
        let value = value.trim();
        !value.is_empty()
            && !value.eq_ignore_ascii_case("none")
            && !value.eq_ignore_ascii_case("not specified")
            && !value.eq_ignore_ascii_case("to be filled by o.e.m.")
    })
}

fn read_sysfs(name: &str) -> Option<String> {
    clean(
        fs::read_to_string(format!("/sys/class/dmi/id/{name}"))
            .ok()
            .map(|value| value.trim().to_string()),
    )
}

fn combined(parts: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    let values: Vec<_> = parts.into_iter().flatten().collect();
    (!values.is_empty()).then(|| values.join(" "))
}

fn bytes(value: u64) -> String {
    to_human_readable_nice(value as f32, &DataType::MemoryBytes)
}

fn disk_kind(kind: Option<i32>) -> Option<&'static str> {
    match kind.and_then(|kind| DiskKind::try_from(kind).ok()) {
        Some(DiskKind::Hdd) => Some("HDD"),
        Some(DiskKind::Ssd) => Some("SSD"),
        Some(DiskKind::NvMe) => Some("NVMe"),
        Some(DiskKind::EMmc) => Some("eMMC"),
        Some(DiskKind::Sd) => Some("SD"),
        Some(DiskKind::IScsi) => Some("iSCSI"),
        Some(DiskKind::Optical) => Some("Optical"),
        Some(DiskKind::Floppy) => Some("Floppy"),
        Some(DiskKind::ThumbDrive) => Some("Thumb Drive"),
        None => None,
    }
}

fn push(entries: &mut Vec<InfoEntry>, name: &str, value: Option<String>) {
    if let Some(value) = clean(value) {
        entries.push(InfoEntry {
            name: name.to_string(),
            value,
        });
    }
}

fn software_sections(about: &About) -> Vec<InfoSection> {
    let os = &about.os_info;
    let de = &about.de_info;
    let device = &about.device_info;
    let mut result = Vec::new();

    let mut system = Vec::new();
    push(&mut system, "Host Name", device.hostname.clone());
    push(
        &mut system,
        "Device",
        combined([device.vendor.clone(), device.model.clone()]),
    );
    push(
        &mut system,
        "Operating System",
        os.pretty_name.clone().or_else(|| os.name.clone()),
    );
    push(
        &mut system,
        "OS Version",
        os.version_id.clone().or_else(|| os.version.clone()),
    );
    push(&mut system, "Architecture", os.os_architecture.clone());
    push(&mut system, "Package Manager", os.package_manager.clone());
    push(
        &mut system,
        "Package Manager Version",
        os.package_manager_version.clone(),
    );
    result.push(InfoSection {
        title: "System".into(),
        entries: system,
    });

    let mut kernel = Vec::new();
    push(&mut kernel, "Kernel", format_kernel_release(os));
    push(&mut kernel, "Build", os.kernel_version.clone());
    result.push(InfoSection {
        title: "Kernel".into(),
        entries: kernel,
    });

    let mut desktop = Vec::new();
    push(&mut desktop, "Desktop", de.desktop_environment.clone());
    push(&mut desktop, "Desktop Version", de.version.clone());
    push(
        &mut desktop,
        "Windowing System",
        de.windowing_system.clone(),
    );
    push(&mut desktop, "Session Type", de.session_type.clone());
    push(
        &mut desktop,
        "Virtual Terminal",
        de.virtual_terminal.clone(),
    );
    result.push(InfoSection {
        title: "Desktop Environment".into(),
        entries: desktop,
    });
    result
}

fn format_kernel_release(os: &OsInfo) -> Option<String> {
    combined([os.os_type.clone(), os.kernel_release.clone()])
}

fn hardware_sections(info: &HardwareInfo) -> Vec<InfoSection> {
    let mut result = Vec::new();

    let mut board = Vec::new();
    push(&mut board, "Manufacturer", read_sysfs("board_vendor"));
    push(&mut board, "Model", read_sysfs("board_name"));
    push(&mut board, "Revision", read_sysfs("board_version"));
    push(
        &mut board,
        "System Product",
        combined([read_sysfs("sys_vendor"), read_sysfs("product_name")]),
    );
    result.push(InfoSection {
        title: "Motherboard".into(),
        entries: board,
    });

    let mut firmware = Vec::new();
    push(&mut firmware, "Vendor", read_sysfs("bios_vendor"));
    push(&mut firmware, "Version", read_sysfs("bios_version"));
    push(&mut firmware, "Release Date", read_sysfs("bios_date"));
    push(&mut firmware, "BIOS Release", read_sysfs("bios_release"));
    result.push(InfoSection {
        title: "BIOS / UEFI".into(),
        entries: firmware,
    });

    let cpu = &info.cpu;
    let mut processor = Vec::new();
    push(&mut processor, "Model", cpu.name.clone());
    if !cpu.core_usage_percent.is_empty() {
        let mut topology = format!("{} logical processors", cpu.core_usage_percent.len());
        if let Some(sockets) = cpu.socket_count.filter(|sockets| *sockets > 0) {
            topology.push_str(&format!(
                ", {sockets} socket{}",
                if sockets == 1 { "" } else { "s" }
            ));
        }
        push(&mut processor, "Topology", Some(topology));
    }
    if cpu.frequency_driver.as_deref() != Some("bogomips") {
        if let Some(freq) = cpu.base_freq_khz.filter(|freq| *freq >= 100_000) {
            push(
                &mut processor,
                "Base Frequency",
                Some(format!("{:.2} GHz", freq as f64 / 1_000_000.0)),
            );
        }
    }
    push(
        &mut processor,
        "Virtualization",
        cpu.virtualization_technology.clone(),
    );
    let caches: Vec<_> = [
        ("L1", cpu.l1_combined_cache_bytes),
        ("L2", cpu.l2_cache_bytes),
        ("L3", cpu.l3_cache_bytes),
    ]
    .into_iter()
    .filter_map(|(name, size)| {
        size.filter(|size| *size > 0)
            .map(|size| format!("{name} {}", bytes(size)))
    })
    .collect();
    push(
        &mut processor,
        "Cache",
        (!caches.is_empty()).then(|| caches.join(" / ")),
    );
    result.push(InfoSection {
        title: "Processor".into(),
        entries: processor,
    });

    let mut memory = Vec::new();
    if info.memory.mem_total > 0 {
        push(&mut memory, "Installed", Some(bytes(info.memory.mem_total)));
    }
    let max_devices = info
        .memory
        .max_devices
        .max(info.memory_devices.len() as u64);
    if max_devices > 0 {
        push(
            &mut memory,
            "Slots",
            Some(format!(
                "{} populated / {max_devices} total",
                info.memory_devices.len(),
            )),
        );
    }
    for (index, module) in info.memory_devices.iter().enumerate() {
        let details = [
            (module.size > 0).then(|| bytes(module.size)),
            clean(Some(module.ram_type.clone())),
            (module.speed > 0).then(|| format!("{} MT/s", module.speed)),
            clean(Some(module.form_factor.clone())),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        push(
            &mut memory,
            &format!("Module {}", index + 1),
            Some(if module.locator.is_empty() {
                details
            } else {
                format!("{} — {details}", module.locator)
            }),
        );
    }
    result.push(InfoSection {
        title: "Memory".into(),
        entries: memory,
    });

    let mut gpus: Vec<_> = info.gpus.values().collect();
    gpus.sort_by(|a, b| a.id.cmp(&b.id));
    let mut graphics = Vec::new();
    for (index, gpu) in gpus.into_iter().enumerate() {
        let vendor = match gpu.vendor_id {
            0x1002 => "AMD",
            0x10de => "NVIDIA",
            0x8086 => "Intel",
            _ => "GPU",
        };
        let mut details = gpu
            .device_name
            .clone()
            .unwrap_or_else(|| format!("{vendor} {:04x}", gpu.device_id));
        if let Some(memory) = gpu.total_memory.filter(|memory| *memory > 0) {
            details.push_str(&format!(" · {} VRAM", bytes(memory)));
        }
        if let (Some(generation), Some(lanes)) = (gpu.max_pcie_gen, gpu.max_pcie_lanes) {
            details.push_str(&format!(" · PCIe {generation}.0 x{lanes}"));
        }
        push(&mut graphics, &format!("GPU {}", index + 1), Some(details));
    }
    result.push(InfoSection {
        title: "Graphics".into(),
        entries: graphics,
    });

    let mut disks: Vec<_> = info.disks.iter().collect();
    disks.sort_by(|a, b| a.id.cmp(&b.id));
    let mut storage = Vec::new();
    for (index, disk) in disks.into_iter().enumerate() {
        let mut parts = Vec::new();
        if let Some(model) = clean(disk.model.clone()) {
            parts.push(model);
        }
        parts.push(bytes(disk.capacity_bytes));
        if let Some(kind) = disk_kind(disk.kind) {
            parts.push(kind.to_string());
        }
        if disk.is_system {
            parts.push("System disk".into());
        }
        push(
            &mut storage,
            &format!("Drive {} ({})", index + 1, disk.id),
            Some(parts.join(" · ")),
        );
    }
    result.push(InfoSection {
        title: "Storage".into(),
        entries: storage,
    });

    result
        .into_iter()
        .filter(|section| !section.entries.is_empty())
        .collect()
}

mod imp {
    use super::*;

    #[derive(gtk::CompositeTemplate, Default)]
    #[template(resource = "/io/missioncenter/MissionCenter/ui/about_system_dialog.ui")]
    pub struct AboutSystemDialog {
        #[template_child]
        logo: TemplateChild<gtk::Image>,
        #[template_child]
        summary_box: TemplateChild<gtk::FlowBox>,
        #[template_child]
        device_title: TemplateChild<gtk::Label>,
        #[template_child]
        device_subtitle: TemplateChild<gtk::Label>,
        #[template_child]
        device_specs: TemplateChild<gtk::Label>,
        #[template_child]
        hardware_groups: TemplateChild<gtk::FlowBox>,
        #[template_child]
        software_groups: TemplateChild<gtk::FlowBox>,
        blur: RefCell<Option<WaylandBlur>>,
    }

    impl AboutSystemDialog {
        fn append_sections(container: &gtk::FlowBox, sections: &[InfoSection]) {
            for section in sections {
                let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
                card.set_hexpand(true);
                card.add_css_class("system-info-card");

                let title = gtk::Label::new(Some(&section.title));
                title.set_xalign(0.5);
                title.set_halign(gtk::Align::Fill);
                title.add_css_class("title-3");
                card.append(&title);

                let group = adw::PreferencesGroup::new();
                for entry in &section.entries {
                    let row = adw::ActionRow::builder()
                        .title(&entry.name)
                        .subtitle(&entry.value)
                        .activatable(true)
                        .build();
                    row.set_subtitle_selectable(true);
                    row.set_subtitle_lines(2);
                    let copy = format!("{}: {}", entry.name, entry.value);
                    row.connect_activated(move |_| {
                        if let Some(display) = gtk::gdk::Display::default() {
                            display.clipboard().set_text(&copy);
                        }
                    });
                    group.add(&row);
                }
                card.append(&group);
                container.append(&card);
            }
        }

        pub(super) fn setup(&self, about: About, hardware: HardwareInfo) {
            let device_title = read_sysfs("board_name")
                .or_else(|| about.device_info.model.clone())
                .or_else(|| hardware.cpu.name.clone())
                .unwrap_or_else(|| "Computer".into());
            self.device_title.set_text(&device_title);
            self.device_subtitle.set_text(
                about
                    .os_info
                    .pretty_name
                    .as_deref()
                    .or(about.os_info.name.as_deref())
                    .unwrap_or("Linux"),
            );

            let mut specs = Vec::new();
            if let Some(cpu) = clean(hardware.cpu.name.clone()) {
                specs.push(cpu);
            }
            if hardware.memory.mem_total > 0 {
                specs.push(format!("{} RAM", bytes(hardware.memory.mem_total)));
            }
            let mut gpu_names: Vec<_> = hardware
                .gpus
                .values()
                .filter_map(|gpu| clean(gpu.device_name.clone()))
                .collect();
            gpu_names.sort();
            specs.extend(gpu_names);
            self.device_specs.set_text(&specs.join("  ·  "));
            self.device_specs.set_visible(!specs.is_empty());

            let hardware = hardware_sections(&hardware);
            let software = software_sections(&about);
            Self::append_sections(&self.hardware_groups, &hardware);
            Self::append_sections(&self.software_groups, &software);

            let logo_visible = about
                .os_info
                .logo
                .map(|image| CachedIcon::from(image).apply_to_image(&self.logo, 144))
                .unwrap_or(false);
            if !logo_visible {
                if let Some(child) = self.logo.parent() {
                    self.summary_box.remove(&child);
                }
            }

            let text = hardware
                .iter()
                .chain(&software)
                .map(|section| {
                    let values = section
                        .entries
                        .iter()
                        .filter(|entry| entry.name != "Host Name")
                        .map(|entry| format!("{}: {}", entry.name, entry.value))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("{}\n{}", section.title, values)
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            let action = gio::SimpleAction::new("copy-all", None);
            action.connect_activate(move |_, _| {
                if let Some(display) = gtk::gdk::Display::default() {
                    display.clipboard().set_text(&text);
                }
            });
            let actions = gio::SimpleActionGroup::new();
            actions.add_action(&action);
            self.obj()
                .insert_action_group("system-info", Some(&actions));
        }

        fn update_blur(&self) {
            let mut blur = self.blur.borrow_mut();
            let Some(blur) = blur.as_mut() else {
                return;
            };
            let window = self.obj();
            if blur.update_full(window.upcast_ref(), true) {
                window.add_css_class("acrylic-enabled");
            } else {
                window.remove_css_class("acrylic-enabled");
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AboutSystemDialog {
        const NAME: &'static str = "AboutSystemDialog";
        type Type = super::AboutSystemDialog;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AboutSystemDialog {}

    impl WidgetImpl for AboutSystemDialog {
        fn realize(&self) {
            self.parent_realize();
            if let Some(blur) = WaylandBlur::new(self.obj().upcast_ref::<gtk::Window>()) {
                self.blur.replace(Some(blur));
                self.update_blur();
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            self.update_blur();
        }

        fn unrealize(&self) {
            if let Some(blur) = self.blur.borrow_mut().as_mut() {
                blur.update_full(self.obj().upcast_ref(), false);
            }
            self.blur.take();
            self.obj().remove_css_class("acrylic-enabled");
            self.parent_unrealize();
        }
    }

    impl WindowImpl for AboutSystemDialog {}
    impl AdwWindowImpl for AboutSystemDialog {}
}

glib::wrapper! {
    pub struct AboutSystemDialog(ObjectSubclass<imp::AboutSystemDialog>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl AboutSystemDialog {
    pub fn new(about: About, hardware: HardwareInfo, parent: &gtk::Window) -> Self {
        let this: Self = glib::Object::builder()
            .property("transient-for", parent)
            .build();
        this.imp().setup(about, hardware);
        this
    }
}
