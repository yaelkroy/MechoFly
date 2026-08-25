#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayAction {
    OpenBrainLab,
    DrosophilaSkin,
    FireflySkin,
    Reevaluate,
    Pause,
    Exit,
}

#[cfg(windows)]
pub struct TrayController {
    _icon: tray_icon::TrayIcon,
    open: tray_icon::menu::MenuItem,
    drosophila: tray_icon::menu::MenuItem,
    firefly: tray_icon::menu::MenuItem,
    reevaluate: tray_icon::menu::MenuItem,
    pause: tray_icon::menu::MenuItem,
    exit: tray_icon::menu::MenuItem,
}

#[cfg(windows)]
impl TrayController {
    pub fn new() -> Result<Self, String> {
        use tray_icon::{
            Icon, TrayIconBuilder,
            menu::{Menu, MenuItem, PredefinedMenuItem},
        };

        let menu = Menu::new();
        let open = MenuItem::new("Open Brain Lab", true, None);
        let drosophila = MenuItem::new("Skin: Drosophila Natural", true, None);
        let firefly = MenuItem::new("Skin: Firefly Field", true, None);
        let reevaluate = MenuItem::new("Re-evaluate capacity", true, None);
        let pause = MenuItem::new("Pause / resume pet", true, None);
        let separator_one = PredefinedMenuItem::separator();
        let separator_two = PredefinedMenuItem::separator();
        let exit = MenuItem::new("Exit MechoFly", true, None);
        menu.append_items(&[
            &open,
            &separator_one,
            &drosophila,
            &firefly,
            &reevaluate,
            &pause,
            &separator_two,
            &exit,
        ])
        .map_err(|error| format!("cannot build tray menu: {error}"))?;
        let icon = Icon::from_rgba(icon_rgba(), 32, 32)
            .map_err(|error| format!("cannot build tray icon: {error}"))?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("MechoFly — modeled connectome companion")
            .with_icon(icon)
            .build()
            .map_err(|error| format!("cannot create tray icon: {error}"))?;
        Ok(Self {
            _icon: icon,
            open,
            drosophila,
            firefly,
            reevaluate,
            pause,
            exit,
        })
    }

    pub fn poll(&self) -> Vec<TrayAction> {
        use tray_icon::menu::MenuEvent;

        MenuEvent::receiver()
            .try_iter()
            .filter_map(|event| {
                if event.id == self.open.id() {
                    Some(TrayAction::OpenBrainLab)
                } else if event.id == self.drosophila.id() {
                    Some(TrayAction::DrosophilaSkin)
                } else if event.id == self.firefly.id() {
                    Some(TrayAction::FireflySkin)
                } else if event.id == self.reevaluate.id() {
                    Some(TrayAction::Reevaluate)
                } else if event.id == self.pause.id() {
                    Some(TrayAction::Pause)
                } else if event.id == self.exit.id() {
                    Some(TrayAction::Exit)
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(windows)]
fn icon_rgba() -> Vec<u8> {
    let mut rgba = vec![0_u8; 32 * 32 * 4];
    for y in 0..32_i32 {
        for x in 0..32_i32 {
            let index = ((y * 32 + x) * 4) as usize;
            let body = ((x - 16) * (x - 16)) / 2 + (y - 17) * (y - 17) < 76;
            let head = (x - 8) * (x - 8) + (y - 17) * (y - 17) < 24;
            let lantern = (x - 25) * (x - 25) + (y - 17) * (y - 17) < 20;
            let wing = ((x - 16) * (x - 16)) / 2 + (y - 8) * (y - 8) < 34;
            let color = if lantern {
                [202, 229, 70, 255]
            } else if head {
                [44, 49, 43, 255]
            } else if body {
                [41, 111, 72, 255]
            } else if wing {
                [176, 215, 217, 170]
            } else {
                [0, 0, 0, 0]
            };
            rgba[index..index + 4].copy_from_slice(&color);
        }
    }
    rgba
}

#[cfg(not(windows))]
pub struct TrayController;

#[cfg(not(windows))]
impl TrayController {
    pub fn new() -> Result<Self, String> {
        Ok(Self)
    }

    pub fn poll(&self) -> Vec<TrayAction> {
        Vec::new()
    }
}
