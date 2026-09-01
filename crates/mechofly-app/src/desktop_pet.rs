//! Windows desktop-pet surface.
//!
//! `eframe` remains the host for Brain Lab and vendor-neutral `wgpu` compute,
//! but its swap-chain transparency is not reliable on Windows.  The pet uses
//! the native layered-window compositor instead: `UpdateLayeredWindow`
//! receives a small premultiplied BGRA bitmap whose zero-alpha pixels are true
//! holes in the desktop surface.

use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::{copy_nonoverlapping, null, null_mut},
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use eframe::egui::{Pos2, Vec2};
use mechofly_core::Behavior;
use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM},
    Graphics::Gdi::{
        AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BLENDFUNCTION, CreateCompatibleDC,
        CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC, HGDIOBJ,
        SelectObject,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey,
            ReleaseCapture, SetCapture, UnregisterHotKey, VK_CONTROL, VK_F12, VK_MENU, VK_SHIFT,
        },
        WindowsAndMessaging::{
            CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA,
            GetCursorPos, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, HTCLIENT,
            HTTRANSPARENT, HWND_TOPMOST, RegisterClassExW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
            SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos, ShowWindow, ULW_ALPHA,
            UpdateLayeredWindow, WM_HOTKEY, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP,
            WM_MOUSEMOVE, WM_NCCREATE, WM_NCHITTEST, WM_RBUTTONUP, WNDCLASSEXW, WS_EX_LAYERED,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        },
    },
};

use crate::pet::{PET_HEIGHT, PET_WIDTH, Skin};

const HIT_ALPHA_THRESHOLD: u8 = 12;
const EVIDENCE_HOLD_MESSAGE: u32 = 0x804D;

const HOTKEY_QUIT: i32 = 0x4D01;
const HOTKEY_VISIBILITY: i32 = 0x4D02;
const HOTKEY_LOOM: i32 = 0x4D03;
const HOTKEY_EMERGENCY_QUIT: i32 = 0x4D04;
const HOTKEY_GROOM: i32 = 0x4D05;
const HOTKEY_REVERSE: i32 = 0x4D06;
const HOTKEY_WALK: i32 = 0x4D07;
const HOTKEY_BRAIN_LAB: i32 = 0x4D08;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotkeyAction {
    Quit,
    ToggleVisibility,
    Loom,
    EmergencyQuit,
    Groom,
    Reverse,
    Walk,
    BrainLab,
}

impl HotkeyAction {
    const fn bit(self) -> u32 {
        match self {
            Self::Quit => 1 << 0,
            Self::ToggleVisibility => 1 << 1,
            Self::Loom => 1 << 2,
            Self::EmergencyQuit => 1 << 3,
            Self::Groom => 1 << 4,
            Self::Reverse => 1 << 5,
            Self::Walk => 1 << 6,
            Self::BrainLab => 1 << 7,
        }
    }
}

#[derive(Clone, Copy)]
struct HotkeyBinding {
    id: i32,
    key: u32,
    modifiers: u32,
    action: HotkeyAction,
    label: &'static str,
}

const HOTKEY_BINDINGS: [HotkeyBinding; 8] = [
    HotkeyBinding {
        id: HOTKEY_QUIT,
        key: b'Q' as u32,
        modifiers: MOD_CONTROL | MOD_ALT,
        action: HotkeyAction::Quit,
        label: "Ctrl+Alt+Q",
    },
    HotkeyBinding {
        id: HOTKEY_VISIBILITY,
        key: b'H' as u32,
        modifiers: MOD_CONTROL | MOD_ALT,
        action: HotkeyAction::ToggleVisibility,
        label: "Ctrl+Alt+H",
    },
    HotkeyBinding {
        id: HOTKEY_LOOM,
        key: b'L' as u32,
        modifiers: MOD_CONTROL | MOD_ALT,
        action: HotkeyAction::Loom,
        label: "Ctrl+Alt+L",
    },
    HotkeyBinding {
        id: HOTKEY_EMERGENCY_QUIT,
        key: VK_F12 as u32,
        modifiers: MOD_CONTROL | MOD_SHIFT,
        action: HotkeyAction::EmergencyQuit,
        label: "Ctrl+Shift+F12",
    },
    HotkeyBinding {
        id: HOTKEY_GROOM,
        key: b'G' as u32,
        modifiers: MOD_CONTROL | MOD_ALT,
        action: HotkeyAction::Groom,
        label: "Ctrl+Alt+G",
    },
    HotkeyBinding {
        id: HOTKEY_REVERSE,
        key: b'B' as u32,
        modifiers: MOD_CONTROL | MOD_ALT,
        action: HotkeyAction::Reverse,
        label: "Ctrl+Alt+B",
    },
    HotkeyBinding {
        id: HOTKEY_WALK,
        key: b'W' as u32,
        modifiers: MOD_CONTROL | MOD_ALT,
        action: HotkeyAction::Walk,
        label: "Ctrl+Alt+W",
    },
    HotkeyBinding {
        id: HOTKEY_BRAIN_LAB,
        key: b'N' as u32,
        modifiers: MOD_CONTROL | MOD_ALT,
        action: HotkeyAction::BrainLab,
        label: "Ctrl+Alt+N",
    },
];

#[derive(Clone, Copy, Debug, Default)]
pub struct PetEvents {
    pub open_lab: bool,
    pub interacted: bool,
    pub dragging: bool,
    pub hovered: bool,
    pub evidence_hold: bool,
    pub position: Option<Pos2>,
    pub cursor_position: Option<Pos2>,
    hotkeys: u32,
}

impl PetEvents {
    pub fn hotkey(self, action: HotkeyAction) -> bool {
        self.hotkeys & action.bit() != 0
    }
}

struct OverlayShared {
    open_lab: AtomicBool,
    interacted: AtomicBool,
    evidence_hold: AtomicBool,
    dragging: Cell<bool>,
    drag_cursor: Cell<(i32, i32)>,
    drag_window: Cell<(i32, i32)>,
    hit_alpha: RefCell<Vec<u8>>,
    hotkeys: AtomicU32,
    hotkey_down: Cell<u32>,
}

impl OverlayShared {
    fn new() -> Self {
        Self {
            open_lab: AtomicBool::new(false),
            interacted: AtomicBool::new(false),
            evidence_hold: AtomicBool::new(false),
            dragging: Cell::new(false),
            drag_cursor: Cell::new((0, 0)),
            drag_window: Cell::new((0, 0)),
            hit_alpha: RefCell::new(vec![0; PET_WIDTH * PET_HEIGHT]),
            hotkeys: AtomicU32::new(0),
            hotkey_down: Cell::new(0),
        }
    }

    fn update_hit_alpha(&self, pixels: &[u8]) {
        let mut hit_alpha = self.hit_alpha.borrow_mut();
        for (destination, pixel) in hit_alpha.iter_mut().zip(pixels.as_chunks::<4>().0) {
            *destination = pixel[3];
        }
    }

    fn hit_test_screen(&self, screen_x: i32, screen_y: i32, rect: &RECT) -> bool {
        let local_x = screen_x - rect.left;
        let local_y = screen_y - rect.top;
        if local_x < 0 || local_y < 0 || local_x >= PET_WIDTH as i32 || local_y >= PET_HEIGHT as i32
        {
            return false;
        }
        let index = local_y as usize * PET_WIDTH + local_x as usize;
        self.hit_alpha.borrow()[index] >= HIT_ALPHA_THRESHOLD
    }
}

pub struct PetOverlay {
    hwnd: HWND,
    memory_dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    bitmap_bits: *mut u8,
    shared: Box<OverlayShared>,
    pixels: Vec<u8>,
    registered_hotkeys: Vec<i32>,
    visible: bool,
    observatory_open: bool,
}

impl PetOverlay {
    pub fn new(position: Pos2, title: &str) -> Result<Self, String> {
        // SAFETY: all handles are checked before use, the registered callback
        // has the required system ABI, and this method runs on the GUI thread.
        unsafe {
            let instance = GetModuleHandleW(null());
            if instance.is_null() {
                return Err(last_error("GetModuleHandleW"));
            }

            let class_name = wide("MechoFlyDesktopPetLayeredWindowV1");
            let mut class: WNDCLASSEXW = zeroed();
            class.cbSize = size_of::<WNDCLASSEXW>() as u32;
            class.style = CS_DBLCLKS;
            class.lpfnWndProc = Some(window_proc);
            class.hInstance = instance;
            class.lpszClassName = class_name.as_ptr();
            if RegisterClassExW(&class) == 0 {
                return Err(last_error("RegisterClassExW"));
            }

            let window_name = wide(title);
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                class_name.as_ptr(),
                window_name.as_ptr(),
                WS_POPUP,
                position.x.round() as i32,
                position.y.round() as i32,
                PET_WIDTH as i32,
                PET_HEIGHT as i32,
                null_mut(),
                null_mut(),
                instance,
                null(),
            );
            if hwnd.is_null() {
                return Err(last_error("CreateWindowExW"));
            }

            let mut shared = Box::new(OverlayShared::new());
            let shared_pointer = shared.as_mut() as *mut OverlayShared;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, shared_pointer as isize);

            let memory_dc = CreateCompatibleDC(null_mut());
            if memory_dc.is_null() {
                DestroyWindow(hwnd);
                return Err(last_error("CreateCompatibleDC"));
            }

            let mut info: BITMAPINFO = zeroed();
            info.bmiHeader.biSize = size_of_val(&info.bmiHeader) as u32;
            info.bmiHeader.biWidth = PET_WIDTH as i32;
            // A negative height creates a top-down DIB: row zero is the
            // visible top edge and needs no copy-time inversion.
            info.bmiHeader.biHeight = -(PET_HEIGHT as i32);
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut bits: *mut c_void = null_mut();
            let bitmap =
                CreateDIBSection(memory_dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                DeleteDC(memory_dc);
                DestroyWindow(hwnd);
                return Err(last_error("CreateDIBSection"));
            }
            let old_bitmap = SelectObject(memory_dc, bitmap);
            if old_bitmap.is_null() {
                DeleteObject(bitmap);
                DeleteDC(memory_dc);
                DestroyWindow(hwnd);
                return Err(last_error("SelectObject"));
            }

            let mut overlay = Self {
                hwnd,
                memory_dc,
                bitmap,
                old_bitmap,
                bitmap_bits: bits.cast(),
                shared,
                pixels: vec![0; PET_WIDTH * PET_HEIGHT * 4],
                registered_hotkeys: Vec::with_capacity(HOTKEY_BINDINGS.len()),
                visible: true,
                observatory_open: false,
            };
            overlay.register_hotkeys();
            overlay.update(
                position,
                Skin::default(),
                Behavior::Rest,
                0.0,
                0.0,
                0.0,
                0.0,
                false,
            )?;
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            Ok(overlay)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        position: Pos2,
        skin: Skin,
        behavior: Behavior,
        phase: f32,
        behavior_age_seconds: f32,
        heading_radians: f32,
        altitude_pixels: f32,
        reduced_motion: bool,
    ) -> Result<(), String> {
        crate::pet::render_pet_bgra_at_age_with_altitude(
            &mut self.pixels,
            skin,
            behavior,
            phase,
            behavior_age_seconds,
            heading_radians,
            altitude_pixels,
            reduced_motion,
        );
        self.shared.update_hit_alpha(&self.pixels);
        // SAFETY: `bitmap_bits` points to a live PET_WIDTH × PET_HEIGHT 32-bit
        // DIB owned by this instance, and both buffers have exactly that size.
        unsafe {
            copy_nonoverlapping(self.pixels.as_ptr(), self.bitmap_bits, self.pixels.len());
            let destination = POINT {
                x: position.x.round() as i32,
                y: position.y.round() as i32,
            };
            let size = SIZE {
                cx: PET_WIDTH as i32,
                cy: PET_HEIGHT as i32,
            };
            let source = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            if UpdateLayeredWindow(
                self.hwnd,
                null_mut(),
                &destination,
                &size,
                self.memory_dc,
                &source,
                0,
                &blend,
                ULW_ALPHA,
            ) == 0
            {
                return Err(last_error("UpdateLayeredWindow"));
            }
        }
        Ok(())
    }

    pub fn poll(&self) -> PetEvents {
        self.poll_hotkey_fallback();
        let position = self.position();
        PetEvents {
            open_lab: self.shared.open_lab.swap(false, Ordering::AcqRel),
            interacted: self.shared.interacted.swap(false, Ordering::AcqRel),
            dragging: self.shared.dragging.get(),
            hovered: self.cursor_hits_pet(),
            evidence_hold: self.shared.evidence_hold.load(Ordering::Acquire),
            position,
            cursor_position: self.cursor_position(),
            hotkeys: self.shared.hotkeys.swap(0, Ordering::AcqRel),
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        // SAFETY: `hwnd` belongs to this object and remains valid until Drop.
        unsafe {
            ShowWindow(self.hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
        self.visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn registered_hotkey_count(&self) -> usize {
        self.registered_hotkeys.len()
    }

    pub fn set_observatory_open(&mut self, open: bool) {
        if self.observatory_open == open {
            return;
        }
        // Live Brain and Brain Lab are diagnostic surfaces; they must not
        // cover the desktop organism. Alpha holes remain click-through.
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        self.observatory_open = open;
    }

    fn register_hotkeys(&mut self) {
        for binding in HOTKEY_BINDINGS {
            // F12 is reserved by Windows for debuggers. It, and any binding
            // owned by another program, still work through the edge-triggered
            // GetAsyncKeyState fallback in `poll_hotkey_fallback`.
            let registered = unsafe {
                RegisterHotKey(
                    self.hwnd,
                    binding.id,
                    binding.modifiers | MOD_NOREPEAT,
                    binding.key,
                ) != 0
                    || RegisterHotKey(self.hwnd, binding.id, binding.modifiers, binding.key) != 0
            };
            if registered {
                self.registered_hotkeys.push(binding.id);
            }
        }
    }

    fn poll_hotkey_fallback(&self) {
        let mut down = 0_u32;
        let previous = self.shared.hotkey_down.get();
        for binding in HOTKEY_BINDINGS {
            let control = binding.modifiers & MOD_CONTROL == 0 || key_down(VK_CONTROL as u32);
            let alt = binding.modifiers & MOD_ALT == 0 || key_down(VK_MENU as u32);
            let shift = binding.modifiers & MOD_SHIFT == 0 || key_down(VK_SHIFT as u32);
            if control && alt && shift && key_down(binding.key) {
                let bit = binding.action.bit();
                down |= bit;
                if previous & bit == 0 {
                    self.shared.hotkeys.fetch_or(bit, Ordering::AcqRel);
                }
            }
        }
        self.shared.hotkey_down.set(down);
    }

    pub fn screen_size(&self) -> Vec2 {
        // SAFETY: GetSystemMetrics has no pointer preconditions.
        let (width, height) = unsafe {
            (
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        };
        Vec2::new(width.max(480) as f32, height.max(320) as f32)
    }

    pub fn screen_origin(&self) -> Pos2 {
        // SAFETY: GetSystemMetrics has no pointer preconditions.
        unsafe {
            Pos2::new(
                GetSystemMetrics(SM_XVIRTUALSCREEN) as f32,
                GetSystemMetrics(SM_YVIRTUALSCREEN) as f32,
            )
        }
    }

    fn position(&self) -> Option<Pos2> {
        // SAFETY: `hwnd` is owned by this object until Drop.
        unsafe {
            let mut rect: RECT = zeroed();
            (GetWindowRect(self.hwnd, &mut rect) != 0)
                .then_some(Pos2::new(rect.left as f32, rect.top as f32))
        }
    }

    fn cursor_position(&self) -> Option<Pos2> {
        // SAFETY: GetCursorPos writes synchronously to the valid stack value.
        unsafe {
            let mut cursor: POINT = zeroed();
            (GetCursorPos(&mut cursor) != 0).then_some(Pos2::new(cursor.x as f32, cursor.y as f32))
        }
    }

    fn cursor_hits_pet(&self) -> bool {
        // SAFETY: `hwnd` is live and both output structures are valid for the
        // duration of these synchronous calls.
        unsafe {
            let mut cursor: POINT = zeroed();
            let mut rect: RECT = zeroed();
            GetCursorPos(&mut cursor) != 0
                && GetWindowRect(self.hwnd, &mut rect) != 0
                && self.shared.hit_test_screen(cursor.x, cursor.y, &rect)
        }
    }
}

impl Drop for PetOverlay {
    fn drop(&mut self) {
        // SAFETY: these handles were created and are uniquely owned by this
        // object. Restoring the previous selected object precedes deletion.
        unsafe {
            for id in &self.registered_hotkeys {
                UnregisterHotKey(self.hwnd, *id);
            }
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            SelectObject(self.memory_dc, self.old_bitmap);
            DeleteObject(self.bitmap);
            DeleteDC(self.memory_dc);
            DestroyWindow(self.hwnd);
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        // The pointer is installed immediately after CreateWindowExW returns.
        // No pet interaction can arrive before then.
        // SAFETY: forwarding an unhandled creation message is required by the
        // window-procedure contract.
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }

    // SAFETY: the user-data slot is either zero or the stable OverlayShared
    // allocation owned by PetOverlay for the entire HWND lifetime.
    let shared_pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const OverlayShared;
    if !shared_pointer.is_null() {
        // SAFETY: see the lifetime argument above; interior mutation uses Cell
        // and atomics because callbacks and polling share the allocation.
        let shared = unsafe { &*shared_pointer };
        match message {
            EVIDENCE_HOLD_MESSAGE => {
                shared.evidence_hold.store(wparam != 0, Ordering::Release);
                return 0;
            }
            WM_HOTKEY => {
                if let Some(binding) = HOTKEY_BINDINGS
                    .iter()
                    .find(|binding| binding.id as usize == wparam)
                {
                    let bit = binding.action.bit();
                    if shared.hotkey_down.get() & bit == 0 {
                        shared.hotkeys.fetch_or(bit, Ordering::AcqRel);
                    }
                }
                return 0;
            }
            WM_NCHITTEST => {
                // lParam stores signed 16-bit screen coordinates. Returning
                // HTTRANSPARENT for an alpha hole lets the real desktop or
                // application below receive the interaction.
                let packed = lparam as u32;
                let screen_x = (packed as u16 as i16) as i32;
                let screen_y = ((packed >> 16) as u16 as i16) as i32;
                // SAFETY: `hwnd` is live for the duration of this callback and
                // `rect` is a valid synchronous output buffer.
                let mut rect: RECT = unsafe { zeroed() };
                let hit = unsafe { GetWindowRect(hwnd, &mut rect) != 0 }
                    && shared.hit_test_screen(screen_x, screen_y, &rect);
                return if hit {
                    HTCLIENT as LRESULT
                } else {
                    HTTRANSPARENT as LRESULT
                };
            }
            WM_LBUTTONDOWN => {
                // SAFETY: valid HWND and stack-allocated output structures.
                unsafe {
                    SetCapture(hwnd);
                    let mut cursor: POINT = zeroed();
                    let mut window: RECT = zeroed();
                    if GetCursorPos(&mut cursor) != 0 && GetWindowRect(hwnd, &mut window) != 0 {
                        shared.drag_cursor.set((cursor.x, cursor.y));
                        shared.drag_window.set((window.left, window.top));
                    }
                }
                shared.dragging.set(true);
                return 0;
            }
            WM_MOUSEMOVE if shared.dragging.get() => {
                // SAFETY: valid HWND and stack-allocated cursor output.
                unsafe {
                    let mut cursor: POINT = zeroed();
                    if GetCursorPos(&mut cursor) != 0 {
                        let (start_x, start_y) = shared.drag_cursor.get();
                        let (window_x, window_y) = shared.drag_window.get();
                        let delta_x = cursor.x - start_x;
                        let delta_y = cursor.y - start_y;
                        SetWindowPos(
                            hwnd,
                            HWND_TOPMOST,
                            window_x + delta_x,
                            window_y + delta_y,
                            0,
                            0,
                            SWP_NOSIZE | SWP_NOACTIVATE,
                        );
                    }
                }
                return 0;
            }
            WM_LBUTTONUP => {
                shared.dragging.set(false);
                shared.interacted.store(true, Ordering::Release);
                // SAFETY: this thread captured the mouse on button down.
                unsafe {
                    ReleaseCapture();
                }
                return 0;
            }
            WM_LBUTTONDBLCLK | WM_RBUTTONUP => {
                shared.open_lab.store(true, Ordering::Release);
                shared.interacted.store(true, Ordering::Release);
                return 0;
            }
            _ => {}
        }
    }

    // SAFETY: all remaining messages are intentionally delegated to the
    // system default procedure with their original values.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn last_error(operation: &str) -> String {
    // SAFETY: GetLastError has no pointer preconditions.
    let code = unsafe { GetLastError() };
    format!("{operation} failed with Windows error {code}")
}

fn key_down(key: u32) -> bool {
    // SAFETY: GetAsyncKeyState has no pointer preconditions.
    unsafe { (GetAsyncKeyState(key as i32) as u16 & 0x8000) != 0 }
}

#[derive(Clone, Debug)]
pub struct HotkeySelfTest {
    pub passed: bool,
    pub binding_count: usize,
    pub registered_count: usize,
    pub labels: Vec<String>,
    pub async_fallback_all_bindings: bool,
}

pub fn run_hotkey_self_test() -> HotkeySelfTest {
    let mut registered = Vec::new();
    let mut unique_ids = std::collections::BTreeSet::new();
    let mut unique_actions = std::collections::BTreeSet::new();
    for (offset, binding) in HOTKEY_BINDINGS.iter().enumerate() {
        unique_ids.insert(binding.id);
        unique_actions.insert(binding.action.bit());
        if binding.action == HotkeyAction::EmergencyQuit {
            continue;
        }
        let id = 0x5D00 + offset as i32;
        // SAFETY: a NULL HWND registers against this short-lived self-test
        // thread; every successful binding is unregistered below.
        if unsafe {
            RegisterHotKey(
                null_mut(),
                id,
                binding.modifiers | MOD_NOREPEAT,
                binding.key,
            )
        } != 0
        {
            registered.push(id);
        }
    }
    for id in &registered {
        // SAFETY: the IDs were registered by this thread just above.
        unsafe {
            UnregisterHotKey(null_mut(), *id);
        }
    }
    HotkeySelfTest {
        passed: unique_ids.len() == HOTKEY_BINDINGS.len()
            && unique_actions.len() == HOTKEY_BINDINGS.len(),
        binding_count: HOTKEY_BINDINGS.len(),
        registered_count: registered.len(),
        labels: HOTKEY_BINDINGS
            .iter()
            .map(|binding| binding.label.to_owned())
            .collect(),
        async_fallback_all_bindings: true,
    }
}
