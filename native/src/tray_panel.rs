//! Native equivalent of CC Switch v4.0.21's Windows tray panel. The existing
//! command IDs remain the single source of truth for all pet operations.
use super::*;
use theme::{fill, font, line, rgb, rounded, smooth_paint, text};

const WIDTH: i32 = 320;
const TAB_QUICK: u32 = 8000;
const TAB_DESKTOP: u32 = 8001;
const GROUP_BASE: u32 = 8100;
const HOVER_TIMER: usize = 0xCC02;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Action,
    Group,
    Toggle,
    Tab,
    Footer,
    Settings,
}
#[derive(Clone)]
struct Item {
    id: u32,
    label: String,
    value: String,
    checked: bool,
    kind: Kind,
    hwnd: HWND,
    parent: HWND,
}
pub(super) struct TrayPanel {
    pub hwnd: HWND,
    submenu: HWND,
    page: u8,
    group: u32,
    hover: HWND,
    scale: f32,
    fonts: [HFONT; 3],
    items: Vec<Item>,
    work: RECT,
    anchor: POINT,
    closing_sub: bool,
}
impl Default for TrayPanel {
    fn default() -> Self {
        Self {
            hwnd: null_mut(),
            submenu: null_mut(),
            page: 0,
            group: 0,
            hover: null_mut(),
            scale: 1.,
            fonts: [null_mut(); 3],
            items: Vec::new(),
            work: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            anchor: POINT { x: 0, y: 0 },
            closing_sub: false,
        }
    }
}
impl Drop for TrayPanel {
    fn drop(&mut self) {
        unsafe {
            for f in self.fonts {
                if !f.is_null() {
                    DeleteObject(f);
                }
            }
        }
    }
}

fn selected<'a>(value: &str, choices: &'a [(&str, &str)]) -> &'a str {
    choices
        .iter()
        .find(|(key, _)| *key == value)
        .map(|(_, label)| *label)
        .unwrap_or("默认")
}
fn checked(a: &App, id: u32) -> bool {
    let s = &a.settings;
    match id {
        110 => s.posture == "standing",
        111 => s.posture == "sitting",
        117 => s.posture == "custom",
        118 => s.main_hand == "right",
        119 => s.main_hand == "left",
        165 => s.main_hand == "mapped",
        166 => s.main_hand == "reverse",
        112 => s.typing_mode == "precise",
        113 => s.typing_mode == "alternating",
        114 => s.arm_model == "classic",
        115 => s.arm_model == "slim",
        120 => s.always_on_top,
        121 => s.click_through,
        122 => s.keyboard_enabled,
        123 => s.track_mouse,
        138 => s.show_cape,
        139 => s.chibi,
        160 => s.autostart,
        140..=143 => (s.scale - [0.75, 1., 1.25, 1.5][(id - 140) as usize]).abs() < 0.01,
        _ => false,
    }
}
fn choices(group: u32) -> (&'static str, Vec<(u32, &'static str)>) {
    match group {
        8100 => (
            "默认姿态",
            vec![(110, "站立"), (111, "坐下"), (117, "自定义姿态")],
        ),
        8101 => (
            "挥手方式",
            vec![
                (118, "右手挥动"),
                (119, "左手挥动"),
                (165, "实际映射"),
                (166, "反向映射"),
            ],
        ),
        8102 => ("敲字模式", vec![(112, "精准按键"), (113, "双手交替")]),
        8103 => ("手臂模型", vec![(114, "经典手臂"), (115, "纤细手臂")]),
        _ => (
            "桌宠大小",
            vec![(140, "75%"), (141, "100%"), (142, "125%"), (143, "150%")],
        ),
    }
}
fn placement(anchor: POINT, work: RECT, size: [i32; 2], margin: i32) -> POINT {
    POINT {
        x: (anchor.x - size[0] / 2).clamp(
            work.left + margin,
            (work.right - size[0] - margin).max(work.left + margin),
        ),
        y: if anchor.y > (work.top + work.bottom) / 2 {
            anchor.y - size[1] - margin
        } else {
            anchor.y + margin
        }
        .clamp(
            work.top + margin,
            (work.bottom - size[1] - margin).max(work.top + margin),
        ),
    }
}
unsafe fn make_window(owner: HWND, anchor: POINT) -> HWND {
    let name = wide("MinecraftTrayPanel");
    let mut class: WNDCLASSEXW = std::mem::zeroed();
    class.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
    class.hInstance = GetModuleHandleW(null());
    class.lpszClassName = name.as_ptr();
    class.lpfnWndProc = Some(window_proc);
    class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    class.style = CS_DROPSHADOW;
    RegisterClassExW(&class);
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_COMPOSITED,
        name.as_ptr(),
        wide("Minecraft 桌宠快捷菜单").as_ptr(),
        WS_POPUP | WS_CLIPCHILDREN,
        // Create on the target monitor so the first placement does not
        // trigger a DPI change and dismiss a newly opened menu.
        anchor.x,
        anchor.y,
        1,
        1,
        owner,
        null_mut(),
        class.hInstance,
        null(),
    );
    if !hwnd.is_null() {
        let corner = 2i32;
        windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            hwnd,
            33,
            &corner as *const _ as *const _,
            4,
        );
    }
    hwnd
}
unsafe fn set_frame(hwnd: HWND, pos: POINT, size: [i32; 2], scale: f32) {
    SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        pos.x,
        pos.y,
        size[0],
        size[1],
        SWP_NOACTIVATE,
    );
    // Windows 10 lacks DWM corner preferences. The region also makes the
    // clipped corner non-interactive on both Windows 10 and Windows 11.
    let radius = (24. * scale).round() as i32;
    let region = CreateRoundRectRgn(0, 0, size[0] + 1, size[1] + 1, radius, radius);
    if SetWindowRgn(hwnd, region, 1) == 0 {
        DeleteObject(region);
    }
}
unsafe fn add(
    a: &mut App,
    parent: HWND,
    id: u32,
    label: &str,
    value: &str,
    kind: Kind,
    is_checked: bool,
    bounds: [i32; 4],
) {
    let [x, y, w, h] = bounds;
    let s = a.tray_ui.scale;
    // Keep HWNDs alive across tab clicks. Destroying the button inside its
    // own BN_CLICKED callback lets its remaining native processing target a
    // recycled HWND (previously another row could become an empty button).
    if let Some(item) = a
        .tray_ui
        .items
        .iter_mut()
        .find(|i| i.parent == parent && i.id == id && i.kind == kind)
    {
        item.checked = is_checked;
        item.label = label.into();
        item.value = value.into();
        let hwnd = item.hwnd;
        SetWindowPos(
            hwnd,
            null_mut(),
            (x as f32 * s).round() as i32,
            (y as f32 * s).round() as i32,
            (w as f32 * s).round() as i32,
            (h as f32 * s).round() as i32,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW | SWP_SHOWWINDOW,
        );
        return;
    }
    let hwnd = CreateWindowExW(
        0,
        wide("BUTTON").as_ptr(),
        wide(label).as_ptr(),
        WS_CHILD | WS_TABSTOP | BS_OWNERDRAW as u32,
        (x as f32 * s).round() as i32,
        (y as f32 * s).round() as i32,
        (w as f32 * s).round() as i32,
        (h as f32 * s).round() as i32,
        parent,
        id as usize as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
    if hwnd.is_null() {
        return;
    }
    a.tray_ui.items.push(Item {
        id,
        label: label.into(),
        value: value.into(),
        checked: is_checked,
        kind,
        hwnd,
        parent,
    });
    // Register before any call that may synchronously request owner drawing.
    SendMessageW(hwnd, WM_SETFONT, a.tray_ui.fonts[0] as usize, 0);
    SetWindowSubclass(hwnd, Some(button_proc), 2, 0);
    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
}
unsafe fn build_main(a: &mut App) {
    close_sub(a);
    let hwnd = a.tray_ui.hwnd;
    SendMessageW(hwnd, WM_SETREDRAW, 0, 0);
    a.tray_ui.hover = null_mut();
    let rows: Vec<_> = a
        .tray_ui
        .items
        .iter()
        .filter(|i| matches!(i.kind, Kind::Action | Kind::Group | Kind::Toggle))
        .map(|i| i.hwnd)
        .collect();
    for child in rows {
        KillTimer(child, HOVER_TIMER);
        SetWindowPos(
            child,
            null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW | SWP_HIDEWINDOW,
        );
    }
    let quick = a.tray_ui.page == 0;
    add(
        a,
        hwnd,
        100,
        "打开控制台",
        "",
        Kind::Settings,
        false,
        [276, 12, 28, 28],
    );
    add(
        a,
        hwnd,
        TAB_QUICK,
        "快捷操作",
        "",
        Kind::Tab,
        quick,
        [12, 52, 148, 30],
    );
    add(
        a,
        hwnd,
        TAB_DESKTOP,
        "桌面设置",
        "",
        Kind::Tab,
        !quick,
        [160, 52, 148, 30],
    );
    if quick {
        let posture = selected(
            &a.settings.posture,
            &[
                ("standing", "站立"),
                ("sitting", "坐下"),
                ("custom", "自定义"),
            ],
        );
        let hand = selected(
            &a.settings.main_hand,
            &[
                ("right", "右手"),
                ("left", "左手"),
                ("mapped", "实际映射"),
                ("reverse", "反向映射"),
            ],
        );
        let mode = if a.settings.typing_mode == "precise" {
            "精准按键"
        } else {
            "双手交替"
        };
        let arm = if a.settings.arm_model == "classic" {
            "经典"
        } else {
            "纤细"
        };
        let size = format!("{}%", (a.settings.scale * 100.).round() as i32);
        for (n, (id, label, value)) in [
            (101, "导入皮肤…", ""),
            (168, "喂食苹果", ""),
            (8100, "默认姿态", posture),
            (8101, "挥手方式", hand),
            (8102, "敲字模式", mode),
            (8103, "手臂模型", arm),
            (8104, "桌宠大小", size.as_str()),
        ]
        .into_iter()
        .enumerate()
        {
            add(
                a,
                hwnd,
                id,
                label,
                value,
                if id >= GROUP_BASE {
                    Kind::Group
                } else {
                    Kind::Action
                },
                false,
                [6, 96 + n as i32 * 38, 308, 36],
            );
        }
    } else {
        for (n, (id, label)) in [
            (120, "保持置顶"),
            (121, "完全鼠标穿透"),
            (122, "全局键盘互动"),
            (123, "视线跟随鼠标"),
            (138, "显示正版披风"),
            (139, "Q 版小人"),
            (160, "随 Windows 登录启动"),
            (130, "把桌宠移到屏幕中央"),
            (
                131,
                if a.hidden {
                    "显示桌宠"
                } else {
                    "暂时隐藏桌宠"
                },
            ),
        ]
        .into_iter()
        .enumerate()
        {
            add(
                a,
                hwnd,
                id,
                label,
                "",
                if n < 7 { Kind::Toggle } else { Kind::Action },
                checked(a, id),
                [6, 96 + n as i32 * 38, 308, 36],
            );
        }
    }
    let footer = if quick { 374 } else { 450 };
    add(
        a,
        hwnd,
        100,
        "打开控制台",
        "",
        Kind::Footer,
        false,
        [6, footer, 214, 36],
    );
    add(
        a,
        hwnd,
        199,
        "退出",
        "",
        Kind::Footer,
        false,
        [224, footer, 90, 36],
    );
    let s = a.tray_ui.scale;
    let size = [
        (WIDTH as f32 * s).round() as i32,
        ((footer + 48) as f32 * s).round() as i32,
    ];
    let pos = placement(
        a.tray_ui.anchor,
        a.tray_ui.work,
        size,
        (8. * s).round() as i32,
    );
    SendMessageW(hwnd, WM_SETREDRAW, 1, 0);
    // WM_SETREDRAW(FALSE) removes WS_VISIBLE. Resizing/moving that hidden
    // window leaves its newly exposed top strip unpainted under composition.
    // Restore painting before changing the frame, then repaint the whole tree.
    set_frame(hwnd, pos, size, s);
    RedrawWindow(
        hwnd,
        null(),
        null_mut(),
        RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW,
    );
}
unsafe fn rebuild_fonts(a: &mut App) {
    let s = a.tray_ui.scale;
    let old = a.tray_ui.fonts;
    a.tray_ui.fonts = [font(13. * s, 400), font(15. * s, 600), font(12. * s, 400)];
    let controls: Vec<_> = a.tray_ui.items.iter().map(|i| i.hwnd).collect();
    for hwnd in controls {
        SendMessageW(hwnd, WM_SETFONT, a.tray_ui.fonts[0] as usize, 0);
    }
    for handle in old {
        if !handle.is_null() {
            DeleteObject(handle);
        }
    }
}

pub(super) unsafe fn open(a: &mut App) {
    if !a.tray_ui.hwnd.is_null() {
        close(a);
        return;
    }
    a.drag = None;
    a.menu_open = true;
    a.panel_dark = a.theme_override.unwrap_or_else(|| theme::system_dark());
    let mut cursor: POINT = std::mem::zeroed();
    GetCursorPos(&mut cursor);
    let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut info) == 0 {
        a.menu_open = false;
        return;
    }
    let (mut dx, mut dy) = (96, 96);
    GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
    let s = (dx as f32 / 96.)
        .min((info.rcWork.bottom - info.rcWork.top - 24) as f32 / 502.)
        .min((info.rcWork.right - info.rcWork.left - 24) as f32 / WIDTH as f32)
        .max(0.5);
    a.tray_ui.scale = s;
    rebuild_fonts(a);
    a.tray_ui.work = info.rcWork;
    a.tray_ui.anchor = cursor;
    a.tray_ui.hwnd = make_window(a.hwnd, a.tray_ui.anchor);
    if a.tray_ui.hwnd.is_null() {
        a.menu_open = false;
        return;
    }
    build_main(a);
    ShowWindow(a.tray_ui.hwnd, SW_SHOW);
    SetForegroundWindow(a.tray_ui.hwnd);
    if let Some(first) = a.tray_ui.items.first() {
        SetFocus(first.hwnd);
    }
}
unsafe fn close_sub(a: &mut App) {
    a.tray_ui.closing_sub = true;
    let hwnd = std::mem::replace(&mut a.tray_ui.submenu, null_mut());
    a.tray_ui.items.retain(|i| i.parent != hwnd);
    if !hwnd.is_null() {
        DestroyWindow(hwnd);
    }
    a.tray_ui.closing_sub = false;
}
pub(super) unsafe fn close(a: &mut App) {
    let main = std::mem::replace(&mut a.tray_ui.hwnd, null_mut());
    close_sub(a);
    a.tray_ui.items.clear();
    a.tray_ui.hover = null_mut();
    a.menu_open = false;
    if !main.is_null() {
        DestroyWindow(main);
    }
}
unsafe fn open_group(a: &mut App, group: u32, keyboard: bool) {
    if a.tray_ui.group == group && !a.tray_ui.submenu.is_null() {
        return;
    }
    close_sub(a);
    a.tray_ui.group = group;
    let (_, options) = choices(group);
    let parent = a.tray_ui.hwnd;
    if parent.is_null() {
        return;
    }
    let submenu = make_window(parent, a.tray_ui.anchor);
    if submenu.is_null() {
        return;
    }
    a.tray_ui.submenu = submenu;
    for (n, (id, label)) in options.iter().enumerate() {
        add(
            a,
            submenu,
            *id,
            label,
            "",
            Kind::Action,
            checked(a, *id),
            [6, 44 + n as i32 * 36, 288, 34],
        );
    }
    let s = a.tray_ui.scale;
    let size = [
        (300. * s).round() as i32,
        ((options.len() as f32 * 36. + 56.) * s).round() as i32,
    ];
    let mut main: RECT = std::mem::zeroed();
    GetWindowRect(parent, &mut main);
    let mut row: RECT = main;
    if let Some(item) = a.tray_ui.items.iter().find(|i| i.id == group) {
        GetWindowRect(item.hwnd, &mut row);
    }
    let work = a.tray_ui.work;
    let gap = (6. * s).round() as i32;
    let x = if main.right + gap + size[0] <= work.right {
        main.right + gap
    } else {
        main.left - gap - size[0]
    };
    let pos = POINT {
        x: x.clamp(work.left, (work.right - size[0]).max(work.left)),
        y: row
            .top
            .clamp(work.top, (work.bottom - size[1]).max(work.top)),
    };
    set_frame(submenu, pos, size, s);
    ShowWindow(submenu, if keyboard { SW_SHOW } else { SW_SHOWNOACTIVATE });
    if keyboard {
        SetForegroundWindow(submenu);
        if let Some(first) = a.tray_ui.items.iter().find(|i| i.parent == submenu) {
            SetFocus(first.hwnd);
        }
    }
}
unsafe fn activate(a: &mut App, id: u32) {
    match id {
        TAB_QUICK | TAB_DESKTOP => {
            a.tray_ui.page = (id - TAB_QUICK) as u8;
            build_main(a);
            if let Some(item) = a.tray_ui.items.iter().find(|i| i.id == id) {
                SetFocus(item.hwnd);
            }
        }
        8100..=8104 => open_group(a, id, true),
        _ => {
            close(a);
            PostMessageW(a.hwnd, WM_COMMAND, id as usize, 0);
        }
    }
}
pub(super) unsafe fn dialog_message(a: &App, msg: &MSG) -> bool {
    for hwnd in [a.tray_ui.submenu, a.tray_ui.hwnd] {
        if !hwnd.is_null() && IsChild(hwnd, msg.hwnd) != 0 {
            if msg.message == WM_KEYDOWN
                && matches!(
                    msg.wParam as u16,
                    VK_ESCAPE | VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_RETURN | VK_SPACE
                )
            {
                SendMessageW(msg.hwnd, msg.message, msg.wParam, msg.lParam);
                return true;
            }
            if IsDialogMessageW(hwnd, msg) != 0 {
                return true;
            }
        }
    }
    false
}
unsafe fn paint(a: &App, hwnd: HWND, dc: HDC) {
    let mut r: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut r);
    smooth_paint(dc, r, |buffer| {
        let dark = a.panel_dark;
        let s = a.tray_ui.scale;
        let q = |n: f32| (n * s).round() as i32;
        let bg = rgb(if dark { 0x242428 } else { 0xebebed });
        let fg = rgb(if dark { 0xe4e4e7 } else { 0x18181b });
        let border = rgb(if dark { 0x36363b } else { 0xd7d7da });
        fill(buffer, &r, bg);
        rounded(
            buffer,
            RECT {
                left: 0,
                top: 0,
                right: r.right,
                bottom: r.bottom,
            },
            q(12.),
            bg,
            Some((border, q(1.))),
        );
        let title = if hwnd == a.tray_ui.hwnd {
            "Minecraft 桌宠"
        } else {
            choices(a.tray_ui.group).0
        };
        text(
            buffer,
            title,
            RECT {
                left: q(16.),
                top: q(10.),
                right: r.right - q(16.),
                bottom: q(40.),
            },
            a.tray_ui.fonts[1],
            fg,
            DT_LEFT,
        );
        if hwnd == a.tray_ui.hwnd {
            let y = if a.tray_ui.page == 0 { 368. } else { 444. };
            line(buffer, q(6.), q(y), r.right - q(6.), q(y), border, q(1.));
            rounded(
                buffer,
                RECT {
                    left: q(12.),
                    top: q(52.),
                    right: q(308.),
                    bottom: q(82.),
                },
                q(8.),
                rgb(if dark { 0x353539 } else { 0xdddddf }),
                None,
            );
        }
    });
}
unsafe fn draw_item(a: &App, draw: &DRAWITEMSTRUCT) {
    let Some(item) = a.tray_ui.items.iter().find(|i| i.hwnd == draw.hwndItem) else {
        return;
    };
    smooth_paint(draw.hDC, draw.rcItem, |dc| {
        let dark = a.panel_dark;
        let s = a.tray_ui.scale;
        let q = |n: f32| (n * s).round() as i32;
        let bg = rgb(if dark { 0x242428 } else { 0xebebed });
        let fg = rgb(if dark { 0xe4e4e7 } else { 0x18181b });
        let muted = rgb(if dark { 0xaaaab2 } else { 0x61616b });
        let blue = rgb(0x3b82f6);
        let mut r = draw.rcItem;
        fill(dc, &r, bg);
        InflateRect(&mut r, -q(1.), -q(1.));
        let hover =
            a.tray_ui.hover == item.hwnd || draw.itemState & (ODS_SELECTED | ODS_FOCUS) != 0;
        let color = if item.kind == Kind::Tab && item.checked {
            rgb(if dark { 0x48484e } else { 0xffffff })
        } else if hover {
            rgb(if dark { 0x37373d } else { 0xdcdce0 })
        } else if item.kind == Kind::Tab {
            rgb(if dark { 0x353539 } else { 0xdddddf })
        } else {
            bg
        };
        rounded(
            dc,
            r,
            q(if item.kind == Kind::Tab { 6. } else { 8. }),
            color,
            None,
        );
        let mut label = r;
        label.left += q(10.);
        label.right -= q(10.);
        if item.kind == Kind::Group {
            label.right = q(145.);
            text(
                dc,
                &item.value,
                RECT {
                    left: q(142.),
                    top: r.top,
                    right: r.right - q(27.),
                    bottom: r.bottom,
                },
                a.tray_ui.fonts[2],
                muted,
                DT_RIGHT,
            );
            let cx = r.right - q(15.);
            let cy = (r.top + r.bottom) / 2;
            line(dc, cx - q(2.), cy - q(4.), cx + q(2.), cy, muted, q(1.));
            line(dc, cx + q(2.), cy, cx - q(2.), cy + q(4.), muted, q(1.));
        } else if item.kind == Kind::Toggle {
            let x = r.right - q(40.);
            let y = (r.top + r.bottom - q(18.)) / 2;
            rounded(
                dc,
                RECT {
                    left: x,
                    top: y,
                    right: x + q(32.),
                    bottom: y + q(18.),
                },
                q(9.),
                if item.checked {
                    blue
                } else {
                    rgb(if dark { 0x505057 } else { 0xbdbdc3 })
                },
                None,
            );
            let left = x + q(if item.checked { 16. } else { 2. });
            rounded(
                dc,
                RECT {
                    left,
                    top: y + q(2.),
                    right: left + q(14.),
                    bottom: y + q(16.),
                },
                q(7.),
                rgb(0xffffff),
                None,
            );
            label.right = x - q(8.);
        } else if item.checked && item.kind != Kind::Tab {
            let x = r.right - q(20.);
            let y = (r.top + r.bottom) / 2;
            line(dc, x - q(5.), y, x - q(1.), y + q(4.), blue, q(2.));
            line(dc, x - q(1.), y + q(4.), x + q(6.), y - q(4.), blue, q(2.));
            label.right = x - q(12.);
        }
        if item.kind == Kind::Settings {
            let cx = (r.left + r.right) / 2;
            let cy = (r.top + r.bottom) / 2;
            let pen = CreatePen(PS_SOLID, q(1.5).max(1), muted);
            let old = SelectObject(dc, pen);
            let brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
            Ellipse(dc, cx - q(5.), cy - q(5.), cx + q(5.), cy + q(5.));
            Ellipse(dc, cx - q(2.), cy - q(2.), cx + q(2.), cy + q(2.));
            for i in 0..8 {
                let angle = i as f32 * std::f32::consts::FRAC_PI_4;
                MoveToEx(
                    dc,
                    cx + q(angle.cos() * 5.),
                    cy + q(angle.sin() * 5.),
                    null_mut(),
                );
                LineTo(dc, cx + q(angle.cos() * 8.), cy + q(angle.sin() * 8.));
            }
            SelectObject(dc, old);
            SelectObject(dc, brush);
            DeleteObject(pen);
        } else {
            text(
                dc,
                &item.label,
                label,
                a.tray_ui.fonts[if item.kind == Kind::Tab { 2 } else { 0 }],
                if item.id == 199 {
                    rgb(if dark { 0xf87171 } else { 0xdc2626 })
                } else {
                    fg
                },
                if matches!(item.kind, Kind::Tab | Kind::Footer) {
                    DT_CENTER
                } else {
                    DT_LEFT
                },
            );
        }
        if draw.itemState & ODS_FOCUS != 0
            && SendMessageW(item.hwnd, WM_QUERYUISTATE, 0, 0) & UISF_HIDEFOCUS as isize == 0
        {
            let pen = CreatePen(PS_SOLID, q(1.).max(1), blue);
            let old = SelectObject(dc, pen);
            let brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
            RoundRect(
                dc,
                r.left + q(1.),
                r.top + q(1.),
                r.right - q(1.),
                r.bottom - q(1.),
                q(12.),
                q(12.),
            );
            SelectObject(dc, old);
            SelectObject(dc, brush);
            DeleteObject(pen);
        }
    });
}
unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if APP.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let a = &mut *APP;
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            paint(a, hwnd, dc);
            EndPaint(hwnd, &ps);
            0
        }
        WM_PRINTCLIENT => {
            paint(a, hwnd, w as HDC);
            0
        }
        WM_DRAWITEM if l != 0 => {
            draw_item(a, &*(l as *const DRAWITEMSTRUCT));
            1
        }
        WM_COMMAND if w >> 16 == BN_CLICKED as usize => {
            activate(a, (w & 0xffff) as u32);
            0
        }
        WM_CLOSE => {
            close(a);
            0
        }
        WM_ACTIVATE if w as u32 & 0xffff == WA_INACTIVE => {
            let next = l as HWND;
            if !a.tray_ui.hwnd.is_null()
                && !a.tray_ui.closing_sub
                && next != a.tray_ui.hwnd
                && (a.tray_ui.submenu.is_null() || next != a.tray_ui.submenu)
            {
                close(a);
            }
            0
        }
        WM_DPICHANGED => {
            close(a);
            0
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED => {
            rebuild_fonts(a);
            a.panel_dark = a.theme_override.unwrap_or_else(|| theme::system_dark());
            for window in [a.tray_ui.hwnd, a.tray_ui.submenu] {
                if !window.is_null() {
                    RedrawWindow(window, null(), null_mut(), RDW_INVALIDATE | RDW_ALLCHILDREN);
                }
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
unsafe extern "system" fn button_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if APP.is_null() {
        return DefSubclassProc(hwnd, msg, w, l);
    }
    let a = &mut *APP;
    match msg {
        WM_MOUSEMOVE => {
            if a.tray_ui.hover != hwnd {
                a.tray_ui.hover = hwnd;
                InvalidateRect(hwnd, null(), 0);
                SetTimer(hwnd, HOVER_TIMER, 140, None);
            }
            let mut t = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut t);
        }
        WM_MOUSELEAVE => {
            if a.tray_ui.hover == hwnd {
                a.tray_ui.hover = null_mut();
            }
            KillTimer(hwnd, HOVER_TIMER);
            InvalidateRect(hwnd, null(), 0);
        }
        WM_TIMER if w == HOVER_TIMER => {
            KillTimer(hwnd, HOVER_TIMER);
            let id = GetDlgCtrlID(hwnd) as u32;
            if (8100..=8104).contains(&id) {
                open_group(a, id, false);
            }
            return 0;
        }
        WM_KEYDOWN => {
            let key = w as u16;
            let parent = GetParent(hwnd);
            match key {
                VK_ESCAPE | VK_LEFT if !a.tray_ui.submenu.is_null() => {
                    let group = a.tray_ui.group;
                    close_sub(a);
                    SetForegroundWindow(a.tray_ui.hwnd);
                    if let Some(row) = a.tray_ui.items.iter().find(|i| i.id == group) {
                        SetFocus(row.hwnd);
                    }
                    return 0;
                }
                VK_ESCAPE => {
                    close(a);
                    return 0;
                }
                VK_UP | VK_DOWN => {
                    let next = GetNextDlgTabItem(parent, hwnd, if key == VK_UP { 1 } else { 0 });
                    SetFocus(next);
                    SendMessageW(
                        parent,
                        WM_CHANGEUISTATE,
                        (UIS_CLEAR | (UISF_HIDEFOCUS << 16)) as usize,
                        0,
                    );
                    return 0;
                }
                VK_RETURN | VK_SPACE => {
                    activate(a, GetDlgCtrlID(hwnd) as u32);
                    return 0;
                }
                VK_RIGHT if (8100..=8104).contains(&(GetDlgCtrlID(hwnd) as u32)) => {
                    open_group(a, GetDlgCtrlID(hwnd) as u32, true);
                    return 0;
                }
                _ => {}
            }
        }
        WM_NCDESTROY => {
            KillTimer(hwnd, HOVER_TIMER);
            RemoveWindowSubclass(hwnd, Some(button_proc), 2);
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, w, l)
}

// Read the presented desktop pixels, not WM_PRINT or a manually composed
// image: those paths conceal stale parent backgrounds and recycled buttons.
unsafe fn verify_presented_page(a: &App, name: &str) -> Result<(), String> {
    let hwnd = a.tray_ui.hwnd;
    let mut rect: RECT = std::mem::zeroed();
    if hwnd.is_null() || GetWindowRect(hwnd, &mut rect) == 0 {
        return Err("托盘切页后窗口失效".into());
    }
    GdiFlush();
    windows_sys::Win32::Graphics::Dwm::DwmFlush();
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    let surface = Surface::new(width, height);
    let screen = GetDC(null_mut());
    if screen.is_null() || surface.bits.is_null() {
        if !screen.is_null() {
            ReleaseDC(null_mut(), screen);
        }
        return Err("无法读取托盘窗口的实际屏幕像素".into());
    }
    let copied = BitBlt(
        surface.dc,
        0,
        0,
        width,
        height,
        screen,
        rect.left,
        rect.top,
        SRCCOPY | CAPTUREBLT,
    );
    ReleaseDC(null_mut(), screen);
    GdiFlush();
    if copied == 0 {
        return Err("托盘窗口屏幕读取失败".into());
    }
    let mut rgba = std::slice::from_raw_parts(surface.bits, (width * height * 4) as usize).to_vec();
    for p in rgba.chunks_exact_mut(4) {
        p.swap(0, 2);
        p[3] = 255;
    }
    image::save_buffer(
        a.store.dir.join(name),
        &rgba,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    )
    .map_err(|e| e.to_string())?;
    let s = a.tray_ui.scale;
    let q = |n: i32| (n as f32 * s).round() as i32;
    let bg: [u8; 3] = if a.panel_dark {
        [36, 36, 40]
    } else {
        [235, 235, 237]
    };
    let mut matching = 0;
    let mut total = 0;
    for y in q(42)..q(48) {
        for x in q(16)..q(260) {
            let p = &rgba[((y * width + x) * 4) as usize..];
            matching += (0..3).all(|c| p[c].abs_diff(bg[c]) < 8) as usize;
            total += 1;
        }
    }
    if matching * 100 < total * 95 {
        return Err(format!("{name}: 托盘标题区域背景丢失 ({matching}/{total})"));
    }
    // The center-pet row was blank after desktop-tab clicks. Require actual
    // foreground text pixels in its label area, independently of background.
    if a.tray_ui.page == 1 {
        let mut ink = 0;
        for y in q(96 + 7 * 38 + 4)..q(96 + 7 * 38 + 32) {
            for x in q(18)..q(180) {
                let p = &rgba[((y * width + x) * 4) as usize..];
                ink += if a.panel_dark { p[0] > 150 } else { p[0] < 100 } as usize;
            }
        }
        if ink < 100 {
            return Err(format!("{name}: 居中操作的文字未绘制 ({ink})"));
        }
    }
    Ok(())
}

pub(super) unsafe fn self_test(a: &mut App) -> Result<(), String> {
    for dark in [false, true] {
        a.theme_override = Some(dark);
        open(a);
        if a.tray_ui.hwnd.is_null() {
            return Err("托盘面板创建失败".into());
        }
        let old_font = a.tray_ui.fonts[0];
        SendMessageW(
            a.tray_ui.hwnd,
            WM_SETTINGCHANGE,
            SPI_SETNONCLIENTMETRICS as usize,
            0,
        );
        if a.tray_ui.fonts[0].is_null() || a.tray_ui.fonts[0] == old_font {
            return Err("托盘未响应系统字体设置通知".into());
        }
        for page in [0, 1] {
            let tab = a
                .tray_ui
                .items
                .iter()
                .find(|i| i.id == TAB_QUICK + page)
                .ok_or("托盘页签不存在")?
                .hwnd;
            SetFocus(tab);
            SendMessageW(tab, BM_CLICK, 0, 0);
            verify_presented_page(
                a,
                &format!(
                    "tray-presented-{}-{page}.png",
                    if dark { "dark" } else { "light" }
                ),
            )?;
            let hwnd = a.tray_ui.hwnd;
            let mut r: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut r);
            let surface = Surface::new(r.right, r.bottom);
            paint(a, hwnd, surface.dc);
            for item in a
                .tray_ui
                .items
                .iter()
                .filter(|i| i.parent == hwnd && IsWindowVisible(i.hwnd) != 0)
            {
                let mut rect: RECT = std::mem::zeroed();
                GetWindowRect(item.hwnd, &mut rect);
                MapWindowPoints(null_mut(), hwnd, &mut rect as *mut RECT as *mut POINT, 2);
                let state = SaveDC(surface.dc);
                SetViewportOrgEx(surface.dc, rect.left, rect.top, null_mut());
                let draw = DRAWITEMSTRUCT {
                    CtlType: ODT_BUTTON,
                    CtlID: item.id,
                    itemID: 0,
                    itemAction: ODA_DRAWENTIRE,
                    itemState: 0,
                    hwndItem: item.hwnd,
                    hDC: surface.dc,
                    rcItem: RECT {
                        left: 0,
                        top: 0,
                        right: rect.right - rect.left,
                        bottom: rect.bottom - rect.top,
                    },
                    itemData: 0,
                };
                draw_item(a, &draw);
                RestoreDC(surface.dc, state);
            }
            GdiFlush();
            let bytes = std::slice::from_raw_parts(surface.bits, (r.right * r.bottom * 4) as usize);
            let mut rgba = bytes.to_vec();
            for pixel in rgba.chunks_exact_mut(4) {
                pixel.swap(0, 2);
                pixel[3] = 255;
            }
            image::save_buffer(
                a.store.dir.join(format!(
                    "tray-{}-page-{page}.png",
                    if dark { "dark" } else { "light" }
                )),
                &rgba,
                r.right as u32,
                r.bottom as u32,
                image::ColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
        }
        for bottom in [false, true] {
            a.tray_ui.anchor = POINT {
                x: (a.tray_ui.work.left + a.tray_ui.work.right) / 2,
                y: if bottom {
                    a.tray_ui.work.bottom - 1
                } else {
                    a.tray_ui.work.top + 1
                },
            };
            for n in 0..20 {
                let page = n % 2;
                let tab = a
                    .tray_ui
                    .items
                    .iter()
                    .find(|i| i.id == TAB_QUICK + page)
                    .ok_or("托盘页签不存在")?
                    .hwnd;
                SetFocus(tab);
                SendMessageW(tab, BM_CLICK, 0, 0);
                if a.tray_ui.page as u32 != page
                    || !a
                        .tray_ui
                        .items
                        .iter()
                        .any(|i| i.id == TAB_QUICK + page && i.hwnd == tab)
                    || a.tray_ui.items.len() != 21
                {
                    return Err("托盘切页后页签被重建、控件重复或页码错误".into());
                }
                verify_presented_page(
                    a,
                    &format!(
                        "tray-presented-{}-{}-{page}.png",
                        if dark { "dark" } else { "light" },
                        if bottom { "bottom" } else { "top" }
                    ),
                )?;
            }
        }
        activate(a, TAB_QUICK);
        for group in 8100..=8104 {
            let row = a
                .tray_ui
                .items
                .iter()
                .find(|i| i.id == group)
                .ok_or("托盘分组按钮不存在")?
                .hwnd;
            SendMessageW(row, WM_KEYDOWN, VK_RIGHT as usize, 0);
            if a.tray_ui.submenu.is_null() {
                return Err(format!("托盘分组菜单创建失败: {group}"));
            }
        }
        let focused = GetFocus();
        SendMessageW(focused, WM_KEYDOWN, VK_ESCAPE as usize, 0);
        if !a.tray_ui.submenu.is_null() || a.tray_ui.hwnd.is_null() {
            return Err(format!(
                "Esc 未仅关闭子菜单 focus={focused:?} now={:?} main={:?} sub={:?}",
                GetFocus(),
                a.tray_ui.hwnd,
                a.tray_ui.submenu
            ));
        }
        let focused = GetFocus();
        SendMessageW(focused, WM_KEYDOWN, VK_ESCAPE as usize, 0);
        if !a.tray_ui.hwnd.is_null() {
            return Err("Esc 未关闭托盘面板".into());
        }
        open(a);
        SendMessageW(a.tray_ui.hwnd, WM_ACTIVATE, WA_INACTIVE as usize, 0);
        if !a.tray_ui.hwnd.is_null() {
            return Err("失去焦点未关闭托盘面板".into());
        }
        open(a);
        activate(a, TAB_DESKTOP);
        let original = a.settings.show_cape;
        let toggle = a
            .tray_ui
            .items
            .iter()
            .find(|i| i.id == 138)
            .ok_or("披风开关不存在")?
            .hwnd;
        SendMessageW(toggle, BM_CLICK, 0, 0);
        let mut msg: MSG = std::mem::zeroed();
        if PeekMessageW(&mut msg, a.hwnd, WM_COMMAND, WM_COMMAND, PM_REMOVE) == 0 {
            return Err("托盘操作没有派发原有命令".into());
        }
        DispatchMessageW(&msg);
        if a.settings.show_cape == original || !a.tray_ui.hwnd.is_null() {
            return Err("托盘操作未更新设置或关闭菜单".into());
        }
        a.command(138);
        close(a);
        if a.menu_open || !a.tray_ui.hwnd.is_null() || !a.tray_ui.submenu.is_null() {
            return Err("托盘菜单未清理".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_stays_inside_each_monitor_work_area() {
        for left in [-1920, 0, 1920] {
            let work = RECT {
                left,
                top: 0,
                right: left + 1920,
                bottom: 1032,
            };
            for x in [left, left + 960, left + 1919] {
                for y in [0, 500, 1031] {
                    let p = placement(POINT { x, y }, work, [400, 628], 10);
                    assert!(
                        p.x >= left
                            && p.y >= 0
                            && p.x + 400 <= work.right
                            && p.y + 628 <= work.bottom
                    );
                }
            }
        }
    }
    #[test]
    fn original_group_commands_are_preserved() {
        let ids: Vec<u32> = (8100..=8104)
            .flat_map(|group| choices(group).1.into_iter().map(|(id, _)| id))
            .collect();
        for id in [
            110, 111, 117, 118, 119, 165, 166, 112, 113, 114, 115, 140, 141, 142, 143,
        ] {
            assert!(ids.contains(&id));
        }
    }
}
