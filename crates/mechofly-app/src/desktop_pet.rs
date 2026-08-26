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
    sync::atomic::{AtomicBool, Ordering},
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
        Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
        WindowsAndMessaging::{
            CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA,
            GetCursorPos, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, HWND_TOPMOST,
            RegisterClassExW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOSIZE,
            SetWindowLongPtrW, SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
            HTCLIENT, HTTRANSPARENT, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP,
            WM_MOUSEMOVE, WM_NCCREATE, WM_NCHITTEST, WM_RBUTTONUP, WNDCLASSEXW,
            WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        },
    },
};

use crate::pet::{PET_HEIGHT, PET_WIDTH, Skin, render_pet_bgra};

const HIT_ALPHA_THRESHOLD: u8 = 12;

#[derive(Clone, Copy, Debug, Default)]
pub struct PetEvents {
    pub open_lab: bool,
    pub interacted: bool,
    pub dragging: bool,
    pub hovered: bool,
    pub position: Option<Pos2>,
}

struct OverlayShared {
    open_lab: AtomicBool,
    interacted: AtomicBool,
    dragging: Cell<bool>,
    drag_cursor: Cell<(i32, i32)>,
    drag_window: Cell<(i32, i32)>,
    hit_alpha: RefCell<Vec<u8>>,
}

impl OverlayShared {
    fn new() -> Self {
        Self {
            open_lab: AtomicBool::new(false),
            interacted: AtomicBool::new(false),
            dragging: Cell::new(false),
            drag_cursor: Cell::new((0, 0)),
            drag_window: Cell::new((0, 0)),
            hit_alpha: RefCell::new(vec![0; PET_WIDTH * PET_HEIGHT]),
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
        if local_x < 0
            || local_y < 0
            || local_x >= PET_WIDTH as i32
            || local_y >= PET_HEIGHT as i32
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
}

impl PetOverlay {
    pub fn new(position: Pos2) -> Result<Self, String> {
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

            let window_name = wide("MechoFly desktop pet");
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
            let bitmap = CreateDIBSection(
                memory_dc,
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                null_mut(),
                0,
            );
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
            };
            overlay.update(
                position,
                Skin::default(),
                Behavior::Rest,
                0.0,
                1.0,
                false,
            )?;
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            Ok(overlay)
        }
    }

    pub fn update(
        &mut self,
        position: Pos2,
        skin: Skin,
        behavior: Behavior,
        phase: f32,
        facing: f32,
        reduced_motion: bool,
    ) -> Result<(), String> {
        render_pet_bgra(
            &mut self.pixels,
            skin,
            behavior,
            phase,
            facing,
            reduced_motion,
        );
        self.shared.update_hit_alpha(&self.pixels);
        // SAFETY: `bitmap_bits` points to a live PET_WIDTH × PET_HEIGHT 32-bit
        // DIB owned by this instance, and both buffers have exactly that size.
        unsafe {
            copy_nonoverlapping(
                self.pixels.as_ptr(),
                self.bitmap_bits,
                self.pixels.len(),
            );
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
        let position = self.position();
        PetEvents {
            open_lab: self.shared.open_lab.swap(false, Ordering::AcqRel),
            interacted: self.shared.interacted.swap(false, Ordering::AcqRel),
            dragging: self.shared.dragging.get(),
            hovered: self.cursor_hits_pet(),
            position,
        }
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
