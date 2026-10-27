use super::*;
use windows_sys::Win32::UI::Controls::Dialogs::*;
const SLIDER_IDS: [i32; 10] = [200, 201, 202, 203, 204, 205, 206, 207, 211, 212];
const SS_PATHELLIPSIS: u32 = 0x8000;
const SS_RIGHT: u32 = 2;
pub(super) const TBM_GETPOS: u32 = WM_USER;
pub(super) const PANEL_WIDTH: f32 = 760.;
pub(super) const PANEL_HEIGHT: f32 = 700.;
pub(super) const GLOBAL_PAGE: u8 = 255;
const PANEL_STYLE: u32 = WS_OVERLAPPED
    | WS_CAPTION
    | WS_SYSMENU
    | WS_MINIMIZEBOX
    | WS_MAXIMIZEBOX
    | WS_THICKFRAME
    | WS_CLIPCHILDREN;

// The default client size is also the minimum; fit it to small displays.
pub(super) fn default_panel_scale(dpi: u32, work: [i32; 2], frame: [i32; 2]) -> f32 {
    (dpi.max(96) as f32 / 96.)
        .min((work[0] - frame[0] - 24).max(1) as f32 / PANEL_WIDTH)
        .min((work[1] - frame[1] - 24).max(1) as f32 / PANEL_HEIGHT)
        .max(0.25)
}

unsafe extern "system" fn panel_wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if APP.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    if msg == WM_NCCREATE {
        (*APP).panel = hwnd;
    }
    panel_proc(&mut *APP, hwnd, msg, w, l)
}

impl App {
    unsafe fn control(
        &mut self,
        class: &str,
        text: &str,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        id: i32,
    ) -> HWND {
        let d = self.panel_scale;
        let hwnd = CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            (x as f32 * d).round() as i32,
            (y as f32 * d).round() as i32,
            (w as f32 * d).round() as i32,
            (h as f32 * d).round() as i32,
            self.panel,
            id as usize as HMENU,
            GetModuleHandleW(null()),
            null(),
        );
        self.layout.push((hwnd, [x, y, w, h]));
        self.page_controls.push((hwnd, self.building_page));
        SendMessageW(hwnd, WM_SETFONT, self.font as usize, 0);
        if id != 0 {
            self.controls.push((id, hwnd));
        }
        if class.eq_ignore_ascii_case("BUTTON") {
            self.panel_ui
                .buttons
                .insert(hwnd as usize, theme::ButtonMotion::default());
            SetWindowSubclass(hwnd, Some(theme::control_proc), 1, 0);
        }
        if class == "EDIT" || class == "STATIC" {
            SetWindowSubclass(hwnd, Some(theme::control_proc), 1, 0);
        }
        hwnd
    }
    unsafe fn text(&mut self, text: &str, x: i32, y: i32, w: i32, role: theme::TextRole) -> HWND {
        let hwnd = self.control("STATIC", text, 0x200, x, y, w, 24, 0);
        self.panel_ui.roles.insert(hwnd as usize, role);
        hwnd
    }
    unsafe fn section(&mut self, title: &str, y: i32) {
        self.text(title, 192, y, 410, theme::TextRole::Section);
    }
    fn card(&mut self, y: i32, h: i32, separators: &[i32]) {
        self.panel_ui.cards.push(theme::Card {
            page: self.building_page,
            bounds: [192, y, 544, h],
            separators: separators.to_vec(),
        });
    }
    unsafe fn switch_row(&mut self, text: &str, y: i32, id: i32) {
        self.check(text, 208, y, 512, id);
    }
    unsafe fn value_slider(&mut self, label: &str, y: i32, id: i32, min: i32, max: i32) {
        self.text(label, 208, y, 136, theme::TextRole::Body);
        self.slider(id, 350, y - 2, 270, min, max);
        let value_id = if id == 200 { 310 } else { 400 + id };
        let hwnd = self.control("STATIC", "", SS_RIGHT | 0x200, 632, y, 88, 24, value_id);
        self.panel_ui
            .roles
            .insert(hwnd as usize, theme::TextRole::Caption);
    }
    pub(super) unsafe fn show_panel(&mut self) {
        if !self.panel.is_null() {
            ShowWindow(self.panel, SW_RESTORE);
            SetForegroundWindow(self.panel);
            return;
        }
        self.apply_theme();
        self.panel_ui.clear_controls();
        self.controls.clear();
        self.layout.clear();
        self.page_controls.clear();
        let class = wide("MinecraftDesktopPetSettings");
        let mut wc: WNDCLASSEXW = std::mem::zeroed();
        wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(panel_wndproc);
        wc.hInstance = GetModuleHandleW(null());
        wc.hIcon = self.icon;
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wc.lpszClassName = class.as_ptr();
        RegisterClassExW(&wc);
        let monitor = MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST);
        let d = self.panel_default_scale(monitor, (self.dpi * 96.) as u32);
        self.panel_scale = d;
        let frame = self.panel_frame((self.dpi * 96.) as u32);
        self.panel = CreateWindowExW(
            WS_EX_ACCEPTFILES | WS_EX_COMPOSITED,
            class.as_ptr(),
            wide(&format!("{} · 控制台", product_title())).as_ptr(),
            PANEL_STYLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            (PANEL_WIDTH * d).round() as i32 + frame[0],
            (PANEL_HEIGHT * d).round() as i32 + frame[1],
            null_mut(),
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        );
        if self.panel.is_null() {
            return;
        }
        self.panel_ui.dpi = GetDpiForWindow(self.panel).max(96);
        self.rebuild_panel_fonts();
        self.building_page = GLOBAL_PAGE;
        for (index, (page, title)) in [
            (0, "外观与皮肤"),
            (3, "交互与动画"),
            (4, "桌面与性能"),
            (1, "视线与角度"),
            (2, "自定义姿态"),
        ]
        .iter()
        .enumerate()
        {
            self.control(
                "BUTTON",
                title,
                BS_RADIOBUTTON as u32 | BS_PUSHLIKE as u32 | WS_TABSTOP,
                12,
                112 + index as i32 * 40,
                144,
                34,
                150 + *page,
            );
        }
        self.control(
            "BUTTON",
            "项目主页",
            BS_PUSHBUTTON as u32 | WS_TABSTOP,
            12,
            582,
            144,
            32,
            169,
        );
        self.control(
            "BUTTON",
            "检查更新",
            BS_PUSHBUTTON as u32 | WS_TABSTOP,
            12,
            624,
            144,
            32,
            170,
        );
        self.status = self.control("STATIC", "所有更改已保存", 0x200, 212, 662, 510, 22, 320);
        self.panel_ui
            .roles
            .insert(self.status as usize, theme::TextRole::Caption);

        // Appearance: the original settings are regrouped without changing IDs
        // or the command/persistence path used by the tray and the pet renderer.
        self.building_page = 0;
        self.section("角色皮肤", 96);
        self.card(128, 164, &[92]);
        self.text("当前皮肤", 208, 142, 320, theme::TextRole::Caption);
        let name = self.control(
            "STATIC",
            "",
            SS_PATHELLIPSIS | 0x200,
            208,
            166,
            360,
            26,
            300,
        );
        self.panel_ui
            .roles
            .insert(name as usize, theme::TextRole::Section);
        self.text(
            "支持 PNG 皮肤，也可以直接拖入窗口",
            208,
            194,
            365,
            theme::TextRole::Caption,
        );
        self.control(
            "BUTTON",
            "导入皮肤…",
            BS_PUSHBUTTON as u32 | WS_TABSTOP,
            594,
            146,
            126,
            32,
            101,
        );
        self.control(
            "BUTTON",
            "恢复内置",
            BS_PUSHBUTTON as u32 | WS_TABSTOP,
            594,
            190,
            126,
            28,
            102,
        );
        self.text("正版 ID", 208, 231, 62, theme::TextRole::Body);
        let input = self.control(
            "EDIT",
            &self.online_id.clone(),
            WS_TABSTOP | ES_AUTOHSCROLL as u32,
            282,
            235,
            280,
            22,
            330,
        );
        SendMessageW(input, EM_SETLIMITTEXT, 36, 0);
        SendMessageW(
            input,
            0x1501,
            0,
            wide("Java 用户名或 UUID").as_ptr() as isize,
        );
        self.control(
            "BUTTON",
            "获取皮肤",
            BS_PUSHBUTTON as u32 | WS_TABSTOP,
            594,
            230,
            126,
            30,
            104,
        );
        let notice = self.control(
            "STATIC",
            &self.online_notice.clone(),
            SS_PATHELLIPSIS | 0x200,
            208,
            263,
            512,
            22,
            333,
        );
        self.panel_ui
            .roles
            .insert(notice as usize, theme::TextRole::Caption);
        self.section("模型", 308);
        self.card(340, 52, &[]);
        self.text("手臂模型", 208, 354, 170, theme::TextRole::Body);
        self.radio("经典 · 4 px", 430, 353, 140, 114);
        self.radio("纤细 · 3 px", 578, 353, 142, 115);
        self.section("显示效果", 408);
        self.card(440, 176, &[40, 80, 120]);
        self.switch_row("显示皮肤外层", 448, 124);
        self.switch_row("显示正版披风", 488, 138);
        self.switch_row("Q 版小人", 528, 139);
        self.value_slider("Q 版身体大小", 582, 212, 50, 180);

        self.building_page = 3;
        self.section("姿势与输入", 96);
        self.card(128, 144, &[48, 96]);
        self.text("默认姿势", 208, 143, 118, theme::TextRole::Body);
        self.radio("站立", 408, 140, 96, 110);
        self.radio("坐下", 516, 140, 96, 111);
        self.radio("自定义", 624, 140, 96, 116);
        self.text("敲字动作", 208, 191, 130, theme::TextRole::Body);
        self.radio("精准按键", 430, 188, 140, 112);
        self.radio("双手交替", 578, 188, 142, 113);
        self.text("挥手方式", 208, 239, 86, theme::TextRole::Body);
        for (id, label, x) in [
            (118, "右手", 300),
            (119, "左手", 408),
            (165, "实际映射", 516),
            (166, "反向映射", 624),
        ] {
            self.radio(label, x, 236, 96, id);
        }
        self.section("互动与动画", 288);
        self.card(320, 288, &[36, 72, 108, 144, 180, 216, 252]);
        for (index, (id, label)) in [
            (122, "全局键盘互动"),
            (137, "hyp 快速队伍聊天"),
            (163, "DA 眨眼 · 需要适配皮肤"),
            (164, "DA 扭腰"),
            (167, "敲字浮现字"),
            (173, "呼吸动画"),
            (172, "可调戏"),
            (171, "播放音乐时冒音符"),
        ]
        .iter()
        .enumerate()
        {
            self.switch_row(label, 324 + index as i32 * 36, *id);
        }
        self.text(
            "站立时按住 Shift 潜行；坐下时跟随键盘敲字。",
            192,
            620,
            544,
            theme::TextRole::Caption,
        );

        self.building_page = 4;
        self.section("桌面行为", 96);
        self.card(128, 120, &[40, 80]);
        self.switch_row("保持置顶", 134, 120);
        self.switch_row("完全鼠标穿透", 174, 121);
        self.switch_row("随 Windows 登录自动启动", 214, 160);
        self.section("显示与性能", 264);
        self.card(296, 180, &[60, 120]);
        self.value_slider("桌宠大小", 314, 200, 65, 180);
        self.value_slider("渲染帧率", 374, 207, 0, 6);
        self.value_slider("打字压低幅度", 434, 211, 0, 30);
        self.section("窗口", 492);
        self.card(524, 90, &[]);
        self.text(
            "自由调整控制台大小",
            208,
            536,
            320,
            theme::TextRole::Section,
        );
        self.text(
            "宽高可分别调整，窗口不能小于默认大小。",
            208,
            562,
            360,
            theme::TextRole::Caption,
        );
        self.control(
            "BUTTON",
            "找回桌宠",
            BS_PUSHBUTTON as u32 | WS_TABSTOP,
            594,
            582,
            126,
            28,
            130,
        );
        self.text(
            "关闭控制台后，桌宠仍会继续运行。",
            192,
            620,
            544,
            theme::TextRole::Caption,
        );

        self.building_page = 1;
        self.section("整体角度", 96);
        self.card(128, 144, &[48, 96]);
        self.control(
            "BUTTON",
            "角度归零",
            BS_PUSHBUTTON as u32 | WS_TABSTOP,
            626,
            92,
            110,
            28,
            133,
        );
        for (axis, label) in ["前后俯仰 · X", "左右转向 · Y", "左右倾斜 · Z"]
            .iter()
            .enumerate()
        {
            self.value_slider(label, 140 + axis as i32 * 48, 208 + axis as i32, -180, 180);
        }
        self.section("视线跟随", 288);
        self.card(320, 300, &[40]);
        self.control(
            "BUTTON",
            "跟随复位",
            BS_PUSHBUTTON as u32 | WS_TABSTOP,
            626,
            284,
            110,
            28,
            132,
        );
        self.switch_row("视线跟随鼠标", 326, 123);
        for (x, base, title) in [(208, 201, "水平 · 左右"), (478, 204, "垂直 · 上下")] {
            self.text(title, x, 370, 226, theme::TextRole::Section);
            for (row, label) in ["灵敏度", "转动范围", "中心偏移"].iter().enumerate() {
                let y = 408 + row as i32 * 56;
                let id = base + row as i32;
                self.text(label, x, y, 130, theme::TextRole::Body);
                let value = self.control(
                    "STATIC",
                    "",
                    SS_RIGHT | 0x200,
                    x + 136,
                    y,
                    100,
                    24,
                    400 + id,
                );
                self.panel_ui
                    .roles
                    .insert(value as usize, theme::TextRole::Caption);
                let (min, max) = match row {
                    0 => (0, 300),
                    1 => (0, 170),
                    _ => (-60, 60),
                };
                self.slider(id, x, y + 26, 236, min, max);
            }
        }
        self.text(
            "负值偏左 / 抬头，正值偏右 / 低头。",
            208,
            588,
            512,
            theme::TextRole::Caption,
        );

        self.building_page = 2;
        self.section("姿态模式", 96);
        self.card(128, 90, &[]);
        self.radio("使用自定义姿态", 208, 140, 242, 117);
        self.radio("站姿", 508, 140, 96, 135);
        self.radio("坐姿", 624, 140, 96, 136);
        self.text(
            "站姿与坐姿分别保存；坐姿在默认动作上调整。",
            208,
            180,
            512,
            theme::TextRole::Caption,
        );
        self.section("关节角度", 234);
        self.card(266, 300, &[40, 82, 124, 166, 208, 250]);
        self.control(
            "BUTTON",
            "姿态归零",
            BS_PUSHBUTTON as u32 | WS_TABSTOP,
            626,
            230,
            110,
            28,
            134,
        );
        for (axis, label) in ["前后 · X", "左右 · Y", "侧向 · Z"].iter().enumerate() {
            self.text(
                label,
                288 + axis as i32 * 144,
                276,
                136,
                theme::TextRole::Caption,
            );
        }
        for (row, name) in ["头部", "身体", "左臂", "右臂", "左腿", "右腿"]
            .iter()
            .enumerate()
        {
            let y = 312 + row as i32 * 42;
            self.text(name, 208, y, 66, theme::TextRole::Body);
            for axis in 0..3 {
                let id = 220 + row as i32 * 3 + axis;
                let x = 282 + axis * 144;
                self.slider(id, x, y - 2, 86, -180, 180);
                let value =
                    self.control("STATIC", "", SS_RIGHT | 0x200, x + 88, y, 50, 24, id + 400);
                self.panel_ui
                    .roles
                    .insert(value as usize, theme::TextRole::Caption);
            }
        }
        self.text(
            "坐姿支持敲字、长按与挥手，互动后回到设定姿态。",
            192,
            586,
            544,
            theme::TextRole::Caption,
        );
        self.text(
            "关闭视线跟随后，头部角度才会生效。",
            192,
            614,
            544,
            theme::TextRole::Caption,
        );
        self.building_page = GLOBAL_PAGE;
        self.rebuild_panel_fonts();
        self.apply_theme();
        self.show_page(self.panel_page);
        self.refresh_panel();
        DragAcceptFiles(self.panel, 1);
        ShowWindow(self.panel, SW_SHOW);
        SetForegroundWindow(self.panel);
    }
    pub(super) unsafe fn show_page(&mut self, page: u8) {
        let page = page.min(4);
        self.panel_page = page;
        let visible = IsWindowVisible(self.panel) != 0;
        if visible {
            SendMessageW(self.panel, WM_SETREDRAW, 0, 0);
        }
        let mut batch = BeginDeferWindowPos(self.page_controls.len() as i32);
        for &(hwnd, owner) in &self.page_controls {
            let show = owner == GLOBAL_PAGE || owner == page;
            if (GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_VISIBLE != 0) == show {
                continue;
            }
            let flags = SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER
                | SWP_NOACTIVATE
                | SWP_NOREDRAW
                | if show { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW };
            if !batch.is_null() {
                batch = DeferWindowPos(batch, hwnd, null_mut(), 0, 0, 0, 0, flags);
            } else {
                SetWindowPos(hwnd, null_mut(), 0, 0, 0, 0, flags);
            }
        }
        if !batch.is_null() {
            EndDeferWindowPos(batch);
        }
        for i in 0..5 {
            SendMessageW(
                self.ctrl(150 + i),
                BM_SETCHECK,
                if i == page as i32 {
                    BST_CHECKED as usize
                } else {
                    BST_UNCHECKED as usize
                },
                0,
            );
        }
        // WM_SETREDRAW(TRUE) also adds WS_VISIBLE, so do not send it to a
        // hidden console while initializing or exporting diagnostic images.
        if visible {
            SendMessageW(self.panel, WM_SETREDRAW, 1, 0);
        }
        RedrawWindow(
            self.panel,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW,
        );
    }
    unsafe fn panel_frame(&self, dpi: u32) -> [i32; 2] {
        let mut rect: RECT = std::mem::zeroed();
        AdjustWindowRectExForDpi(
            &mut rect,
            PANEL_STYLE,
            0,
            WS_EX_ACCEPTFILES | WS_EX_COMPOSITED,
            dpi.max(96),
        );
        [rect.right - rect.left, rect.bottom - rect.top]
    }
    pub(super) unsafe fn panel_default_scale(&self, monitor: HMONITOR, dpi: u32) -> f32 {
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return dpi.max(96) as f32 / 96.;
        }
        default_panel_scale(
            dpi,
            [
                info.rcWork.right - info.rcWork.left,
                info.rcWork.bottom - info.rcWork.top,
            ],
            self.panel_frame(dpi),
        )
    }
    unsafe fn panel_resize(&mut self) {
        if self.panel.is_null() || IsIconic(self.panel) != 0 {
            return;
        }
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(self.panel, &mut client);
        if client.right <= 0 || client.bottom <= 0 {
            return;
        }
        let dpi = GetDpiForWindow(self.panel).max(96);
        let scale =
            self.panel_default_scale(MonitorFromWindow(self.panel, MONITOR_DEFAULTTONEAREST), dpi);
        self.panel_scale = scale;
        self.panel_ui.dpi = dpi;
        self.panel_ui.client = [client.right, client.bottom];
        let mut batch = BeginDeferWindowPos(self.layout.len() as i32);
        for &(hwnd, bounds) in &self.layout {
            let r = self.panel_rect(bounds);
            if !batch.is_null() {
                batch = DeferWindowPos(
                    batch,
                    hwnd,
                    null_mut(),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            } else {
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
        if !batch.is_null() {
            EndDeferWindowPos(batch);
        }
        self.rebuild_panel_fonts();
        RedrawWindow(
            self.panel,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN,
        );
    }
    unsafe fn panel_dpi(&mut self, hwnd: HWND, w: WPARAM, l: LPARAM) {
        if l == 0 {
            return;
        }
        let proposed = *(l as *const RECT);
        let dpi = (w & 0xffff) as u32;
        let frame = self.panel_frame(dpi);
        let minimum =
            self.panel_default_scale(MonitorFromRect(&proposed, MONITOR_DEFAULTTONEAREST), dpi);
        self.panel_ui.dpi = dpi;
        SetWindowPos(
            hwnd,
            null_mut(),
            proposed.left,
            proposed.top,
            (proposed.right - proposed.left).max((PANEL_WIDTH * minimum).round() as i32 + frame[0]),
            (proposed.bottom - proposed.top)
                .max((PANEL_HEIGHT * minimum).round() as i32 + frame[1]),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        self.panel_resize();
    }
    pub(super) unsafe fn github_icon(&mut self) {
        let size = (18. * self.panel_scale).round() as i32;
        let icon = LoadImageW(
            GetModuleHandleW(null()),
            (if self.panel_dark { 3usize } else { 2usize }) as *const u16,
            IMAGE_ICON,
            size,
            size,
            0,
        );
        if icon.is_null() {
            return;
        }
        let images = ImageList_Create(size, size, ILC_COLOR32, 1, 0);
        if images == 0 {
            DestroyIcon(icon);
            return;
        }
        ImageList_ReplaceIcon(images, -1, icon);
        DestroyIcon(icon);
        let margin = (8. * self.panel_scale).round() as i32;
        let data = BUTTON_IMAGELIST {
            himl: images,
            margin: RECT {
                left: margin,
                top: 0,
                right: margin,
                bottom: 0,
            },
            uAlign: BUTTON_IMAGELIST_ALIGN_LEFT,
        };
        SendMessageW(
            self.ctrl(169),
            BCM_SETIMAGELIST,
            0,
            &data as *const _ as isize,
        );
        if self.github_images != 0 {
            ImageList_Destroy(self.github_images);
        }
        self.github_images = images;
    }
    unsafe fn radio(&mut self, text: &str, x: i32, y: i32, w: i32, id: i32) {
        self.control(
            "BUTTON",
            text,
            BS_RADIOBUTTON as u32 | BS_PUSHLIKE as u32 | WS_TABSTOP,
            x,
            y,
            w,
            28,
            id,
        );
    }
    unsafe fn check(&mut self, text: &str, x: i32, y: i32, w: i32, id: i32) {
        self.control(
            "BUTTON",
            text,
            BS_CHECKBOX as u32 | WS_TABSTOP,
            x,
            y,
            w,
            28,
            id,
        );
    }
    unsafe fn slider(&mut self, id: i32, x: i32, y: i32, w: i32, min: i32, max: i32) {
        let h = self.control(
            "msctls_trackbar32",
            "",
            TBS_HORZ
                | (if id == 207 {
                    TBS_AUTOTICKS
                } else {
                    TBS_NOTICKS
                })
                | WS_TABSTOP,
            x,
            y,
            w,
            29,
            id,
        );
        SendMessageW(h, TBM_SETRANGEMIN, 0, min as isize);
        SendMessageW(h, TBM_SETRANGEMAX, 1, max as isize);
        SendMessageW(h, TBM_SETPAGESIZE, 0, if id == 207 { 1 } else { 5 });
    }
    fn ctrl(&self, id: i32) -> HWND {
        self.controls
            .iter()
            .find(|p| p.0 == id)
            .map(|p| p.1)
            .unwrap_or(null_mut())
    }
    pub(super) unsafe fn refresh_panel(&mut self) {
        if self.panel.is_null() {
            return;
        }
        EnableWindow(self.ctrl(104), if self.download.is_some() { 0 } else { 1 });
        SetWindowTextW(
            self.ctrl(104),
            wide(if self.download.is_some() {
                "获取中…"
            } else {
                "获取皮肤"
            })
            .as_ptr(),
        );
        SetWindowTextW(self.ctrl(333), wide(&self.online_notice).as_ptr());
        for (id, checked) in [
            (110, self.settings.posture == "standing"),
            (111, self.settings.posture == "sitting"),
            (116, self.settings.posture == "custom"),
            (117, self.settings.posture == "custom"),
            (135, self.settings.custom_base == "standing"),
            (136, self.settings.custom_base == "sitting"),
            (118, self.settings.main_hand == "right"),
            (119, self.settings.main_hand == "left"),
            (112, self.settings.typing_mode == "precise"),
            (113, self.settings.typing_mode == "alternating"),
            (114, self.settings.arm_model == "classic"),
            (115, self.settings.arm_model == "slim"),
            (120, self.settings.always_on_top),
            (121, self.settings.click_through),
            (122, self.settings.keyboard_enabled),
            (137, self.settings.hyp_quick_chat),
            (123, self.settings.track_mouse),
            (124, self.settings.outer_layer),
            (138, self.settings.show_cape),
            (139, self.settings.chibi),
            (163, self.settings.da_blink && !self.settings.chibi),
            (164, self.settings.da_dance),
            (165, self.settings.main_hand == "mapped"),
            (166, self.settings.main_hand == "reverse"),
            (167, self.settings.typing_letters),
            (171, self.settings.music_notes),
            (172, self.settings.tease_enabled),
            (173, self.settings.breathing_animation),
            (160, self.settings.autostart),
        ] {
            SendMessageW(
                self.ctrl(id),
                BM_SETCHECK,
                if checked {
                    BST_CHECKED as usize
                } else {
                    BST_UNCHECKED as usize
                },
                0,
            );
        }
        EnableWindow(self.ctrl(163), if self.settings.chibi { 0 } else { 1 });
        EnableWindow(self.ctrl(212), if self.settings.chibi { 1 } else { 0 });
        SetWindowTextW(self.ctrl(300), wide(&self.settings.skin_name).as_ptr());
        let values = [
            (self.settings.scale * 100.).round() as i32,
            self.settings.head_sensitivity_x as i32,
            (self.settings.head_range_x * 2.).round() as i32,
            self.settings.head_center_x as i32,
            self.settings.head_sensitivity_y as i32,
            (self.settings.head_range_y * 2.).round() as i32,
            self.settings.head_center_y as i32,
            config::FPS_OPTIONS
                .iter()
                .position(|&fps| fps == self.settings.fps)
                .unwrap_or(1) as i32,
            self.settings.typing_squash.round() as i32,
            self.settings.chibi_body_scale.round() as i32,
        ];
        for (i, &id) in SLIDER_IDS.iter().enumerate() {
            SendMessageW(self.ctrl(id), TBM_SETPOS, 1, values[i] as isize);
            let text = match id {
                200 | 201 | 204 | 211 | 212 => format!("{}%", values[i]),
                202 | 205 => format!("±{}°", values[i] as f32 / 2.),
                207 => format!("{} FPS", self.settings.fps),
                _ => format!("{:+}°", values[i]),
            };
            SetWindowTextW(
                self.ctrl(if id == 200 { 310 } else { 400 + id }),
                wide(&text).as_ptr(),
            );
        }
        for axis in 0..3 {
            let value = self.settings.model_rotation[axis] as i32;
            SendMessageW(self.ctrl(208 + axis as i32), TBM_SETPOS, 1, value as isize);
            SetWindowTextW(
                self.ctrl(608 + axis as i32),
                wide(&format!("{value:+}°")).as_ptr(),
            );
        }
        for row in 0..6 {
            for axis in 0..3 {
                let id = 220 + row as i32 * 3 + axis as i32;
                let value = self.settings.active_custom_pose().row(row)[axis] as i32;
                SendMessageW(self.ctrl(id), TBM_SETPOS, 1, value as isize);
                SetWindowTextW(self.ctrl(id + 400), wide(&format!("{value:+}°")).as_ptr());
            }
        }
        SetWindowTextW(
            self.status,
            wide(if self.notice.is_empty() || self.notice == "已保存" {
                "所有更改已保存"
            } else {
                &self.notice
            })
            .as_ptr(),
        );
    }
    unsafe fn slide(&mut self, hwnd: HWND, tracking: bool) {
        let id = GetDlgCtrlID(hwnd);
        let value = SendMessageW(hwnd, TBM_GETPOS, 0, 0) as f32;
        match id {
            200 => {
                self.settings.scale = value / 100.;
                self.resize_pet();
            }
            201 => self.settings.head_sensitivity_x = value,
            202 => self.settings.head_range_x = value / 2.,
            203 => self.settings.head_center_x = value,
            204 => self.settings.head_sensitivity_y = value,
            205 => self.settings.head_range_y = value / 2.,
            206 => self.settings.head_center_y = value,
            207 => {
                self.settings.fps = config::FPS_OPTIONS[(value as usize).min(6)];
                self.next_draw = self.start.elapsed().as_secs_f64();
            }
            208..=210 => self.settings.model_rotation[(id - 208) as usize] = value,
            211 => self.settings.typing_squash = value,
            212 => self.settings.chibi_body_scale = value,
            220..=237 => {
                self.settings
                    .active_custom_pose_mut()
                    .row_mut((id - 220) as usize / 3)[(id - 220) as usize % 3] = value;
                if self.settings.posture != "custom" {
                    self.reset_keys();
                    self.pose.reset_swing();
                    self.settings.posture = "custom".into();
                    self.pose.sit = if self.settings.is_sitting() { 1. } else { 0. };
                }
            }
            _ => return,
        }
        self.settings.clean();
        self.wake();
        let text = match id {
            200 | 201 | 204 | 211 | 212 => format!("{}%", value as i32),
            202 | 205 => format!("±{}°", value / 2.),
            207 => format!("{} FPS", self.settings.fps),
            _ => format!("{:+}°", value as i32),
        };
        SetWindowTextW(
            self.ctrl(if id == 200 { 310 } else { id + 400 }),
            wide(&text).as_ptr(),
        );
        if tracking {
            if !self.slider_pending {
                SetWindowTextW(self.status, wide("调整中，松手保存").as_ptr());
            }
            self.slider_pending = true;
        } else {
            self.save();
            self.refresh_panel();
        }
    }
    pub(super) unsafe fn fetch_skin(&mut self) {
        if self.download.is_some() {
            return;
        }
        let mut text = [0u16; 64];
        let len = GetWindowTextW(self.ctrl(330), text.as_mut_ptr(), 64);
        let id = String::from_utf16_lossy(&text[..len as usize])
            .trim()
            .to_owned();
        if let Err(e) = online::identity(&id) {
            self.online_notice = e;
            self.refresh_panel();
            return;
        }
        self.online_id = id.clone();
        self.online_notice = "正在连接官方皮肤服务…".into();
        let (tx, rx) = std::sync::mpsc::channel();
        self.download = Some(rx);
        match std::thread::Builder::new()
            .name("skin-download".into())
            .spawn(move || {
                let _ = tx.send(online::fetch(&id));
            }) {
            Ok(_) => {}
            Err(e) => {
                self.download = None;
                self.online_notice = format!("无法开始下载：{e}");
            }
        }
        self.refresh_panel();
    }
    pub(super) unsafe fn poll_download(&mut self) {
        let result = match self.download.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(result)) => result,
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                Err("下载已中断，请重试。".into())
            }
            _ => return,
        };
        self.download = None;
        match result {
            Ok(result) => {
                let dest = self.store.dir.join("skin.png");
                if let Err(e) = config::atomic(&dest, &result.bytes) {
                    self.online_notice = format!("皮肤保存失败：{e}");
                    self.refresh_panel();
                    return;
                }
                self.skin = result.skin;
                self.settings.skin_path = dest.to_string_lossy().into_owned();
                // 正版披风：拿到就存成 cape.png，没有就清掉旧披风。
                let mut cape_notice = String::new();
                match result.cape {
                    Some(bytes) => {
                        let cape_dest = self.store.dir.join("cape.png");
                        if let Ok(cape) = Cape::load(&bytes) {
                            if config::atomic(&cape_dest, &bytes).is_ok() {
                                self.settings.cape_path = cape_dest.to_string_lossy().into_owned();
                                self.cape = Some(cape);
                                cape_notice = "，含披风".into();
                            }
                        }
                    }
                    None => {
                        self.settings.cape_path.clear();
                        self.cape = None;
                        cape_notice = "，该账号没有披风".into();
                    }
                }
                self.settings.skin_name = format!("{} · 正版皮肤", result.name);
                self.settings.arm_model = if result.slim && !self.skin.legacy {
                    "slim"
                } else {
                    "classic"
                }
                .into();
                self.changed();
                self.online_notice = format!(
                    "已获取 {} 的皮肤{cape_notice}，已自动识别手臂类型。",
                    result.name
                );
            }
            Err(e) => self.online_notice = e,
        }
        self.refresh_panel();
    }
    pub(super) unsafe fn choose_skin(&mut self) {
        let mut buffer = [0u16; 32768];
        let filter = wide("Minecraft PNG 皮肤\0*.png\0\0");
        let mut data: OPENFILENAMEW = std::mem::zeroed();
        data.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        data.hwndOwner = if self.panel.is_null() {
            self.hwnd
        } else {
            self.panel
        };
        data.lpstrFilter = filter.as_ptr();
        data.lpstrFile = buffer.as_mut_ptr();
        data.nMaxFile = buffer.len() as u32;
        let title = wide("选择 Minecraft 皮肤");
        data.lpstrTitle = title.as_ptr();
        data.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
        if GetOpenFileNameW(&mut data) != 0 {
            let len = buffer.iter().position(|&c| c == 0).unwrap();
            self.import(PathBuf::from(String::from_utf16_lossy(&buffer[..len])));
        }
    }
}
pub(super) unsafe fn panel_proc(
    a: &mut App,
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
) -> LRESULT {
    if let Some(result) = a.theme_message(hwnd, msg, w, l) {
        return result;
    }
    match msg {
        WM_GETMINMAXINFO => {
            // Keep DefWindowProc's normal maximize bounds; width and height
            // have independent minimums rather than an aspect constraint.
            let result = DefWindowProcW(hwnd, msg, w, l);
            if l != 0 {
                let limits = &mut *(l as *mut MINMAXINFO);
                let dpi = GetDpiForWindow(hwnd).max(96);
                let frame = a.panel_frame(dpi);
                let minimum =
                    a.panel_default_scale(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), dpi);
                limits.ptMinTrackSize = POINT {
                    x: (PANEL_WIDTH * minimum).round() as i32 + frame[0],
                    y: (PANEL_HEIGHT * minimum).round() as i32 + frame[1],
                };
            }
            result
        }
        WM_SIZE => {
            a.panel_resize();
            0
        }
        WM_ENTERSIZEMOVE => 0,
        WM_EXITSIZEMOVE => {
            a.panel_resize();
            if a.diagnostic {
                diagnostic_snapshot(a);
            }
            0
        }
        WM_TIMER if w == theme::ANIMATION_TIMER => {
            a.panel_animation_tick();
            0
        }
        WM_COMMAND => {
            let id = (w & 0xffff) as u32;
            a.command(id);
            0
        }
        WM_HSCROLL => {
            if l != 0 {
                a.slide(l as HWND, (w & 0xffff) as u32 == TB_THUMBTRACK);
                // The antialiased round thumb extends beyond the native
                // trackbar's narrow thumb rectangle. Clear the entire control
                // on movement so native partial invalidation cannot leave arcs.
                InvalidateRect(l as HWND, null(), 0);
            }
            0
        }
        WM_DROPFILES => {
            drop_skin(a, w);
            0
        }
        WM_CTLCOLORSTATIC => {
            let dc = w as HDC;
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextColor(dc, 0x00252525);
            GetStockObject(WHITE_BRUSH) as isize
        }
        WM_CTLCOLORBTN => GetStockObject(WHITE_BRUSH) as isize,
        WM_QUERYENDSESSION => {
            a.save();
            1
        }
        WM_ENDSESSION => {
            if w != 0 {
                a.save();
            }
            0
        }
        WM_DPICHANGED => {
            a.panel_dpi(hwnd, w, l);
            0
        }
        WM_CLOSE => {
            a.save();
            DestroyWindow(hwnd);
            // 关闭控制台后如果托盘图标此前没加上，再重试一轮。
            if !a.tray_ok {
                a.tray_retry = 0;
                a.tray_retry_at = 0.;
            }
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, theme::ANIMATION_TIMER);
            a.panel_ui.clear_controls();
            a.panel = null_mut();
            a.status = null_mut();
            a.controls.clear();
            a.layout.clear();
            a.page_controls.clear();
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
pub(super) unsafe fn drop_skin(a: &mut App, w: WPARAM) {
    let drop = w as HDROP;
    let len = DragQueryFileW(drop, 0, null_mut(), 0);
    let mut name = vec![0u16; len as usize + 1];
    DragQueryFileW(drop, 0, name.as_mut_ptr(), len + 1);
    DragFinish(drop);
    if len > 0 {
        a.import(PathBuf::from(String::from_utf16_lossy(
            &name[..len as usize],
        )));
    }
}
pub(super) unsafe fn diagnostic_init(a: &mut App) {
    a.show_panel();
    diagnostic_snapshot(a);
}
unsafe fn save_panel_image(a: &App, name: &str) -> Result<(), String> {
    let surface = a.capture_panel().ok_or("控制台图像缓冲区创建失败")?;
    GdiFlush();
    let pixels =
        std::slice::from_raw_parts(surface.bits, surface.w as usize * surface.h as usize * 4);
    let mut rgba = Vec::with_capacity(pixels.len());
    for pixel in pixels.chunks_exact(4) {
        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
    }
    image::save_buffer(
        a.store.dir.join(name),
        &rgba,
        surface.w as u32,
        surface.h as u32,
        image::ColorType::Rgba8,
    )
    .map_err(|e| e.to_string())
}

// Runs only with an explicitly isolated diagnostic profile. Exercise the real
// Win32 controls, message routing and layout, then export their own rendering.
unsafe fn capture_presented_control(hwnd: HWND, path: &std::path::Path) -> Result<Vec<u8>, String> {
    let mut rect: RECT = std::mem::zeroed();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return Err("无法读取滑块位置".into());
    }
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    let surface = Surface::new(width, height);
    let screen = GetDC(null_mut());
    if surface.bits.is_null() || screen.is_null() {
        if !screen.is_null() {
            ReleaseDC(null_mut(), screen);
        }
        return Err("无法读取滑块屏幕像素".into());
    }
    GdiFlush();
    windows_sys::Win32::Graphics::Dwm::DwmFlush();
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
        return Err("滑块屏幕截图失败".into());
    }
    let mut rgba = std::slice::from_raw_parts(surface.bits, (width * height * 4) as usize).to_vec();
    for p in rgba.chunks_exact_mut(4) {
        p.swap(0, 2);
        p[3] = 255;
    }
    image::save_buffer(
        path,
        &rgba,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    )
    .map_err(|e| e.to_string())?;
    Ok(rgba)
}

unsafe fn verify_slider_repaint(a: &App, slider: HWND, tag: &str) -> Result<(), String> {
    let before = capture_presented_control(slider, &a.store.dir.join(format!("slider-{tag}.png")))?;
    RedrawWindow(
        slider,
        null(),
        null_mut(),
        RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW,
    );
    let after = capture_presented_control(
        slider,
        &a.store.dir.join(format!("slider-{tag}-repaint.png")),
    )?;
    let changed = before
        .chunks_exact(4)
        .zip(after.chunks_exact(4))
        .filter(|(x, y)| x[..3] != y[..3])
        .count();
    if changed > 8 {
        return Err(format!(
            "Q版大小滑块残影 ({tag}): 全量重绘改变 {changed} 个像素"
        ));
    }
    Ok(())
}

unsafe fn slider_drag_self_test(a: &mut App) -> Result<(), String> {
    const LEFT_BUTTON_DOWN: WPARAM = 0x0001; // MK_LBUTTON in mouse-message wParam.
    let original = (a.settings.chibi, a.settings.chibi_body_scale);
    a.settings.chibi = true;
    a.show_page(0);
    a.refresh_panel();
    let slider = a.ctrl(212);
    SetForegroundWindow(a.panel);
    SetFocus(slider);
    SendMessageW(
        a.panel,
        WM_CHANGEUISTATE,
        (UIS_CLEAR | (UISF_HIDEFOCUS << 16)) as usize,
        0,
    );
    for dark in [false, true] {
        a.theme_override = Some(dark);
        a.apply_theme();
        SendMessageW(slider, TBM_SETPOS, 1, 100);
        RedrawWindow(
            a.panel,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW,
        );
        let mut channel: RECT = std::mem::zeroed();
        SendMessageW(
            slider,
            TBM_GETCHANNELRECT,
            0,
            &mut channel as *mut _ as isize,
        );
        let tag = if dark { "dark" } else { "light" };
        for (direction, end) in [("right", channel.right - 8), ("left", channel.left + 8)] {
            let mut thumb: RECT = std::mem::zeroed();
            SendMessageW(slider, TBM_GETTHUMBRECT, 0, &mut thumb as *mut _ as isize);
            let y = (thumb.top + thumb.bottom) / 2;
            let start = (thumb.left + thumb.right) / 2;
            let coords = |x: i32| ((y as u32) << 16 | (x as u32 & 0xffff)) as isize;
            SendMessageW(slider, WM_LBUTTONDOWN, LEFT_BUTTON_DOWN, coords(start));
            let step = if end >= start { 1 } else { -1 };
            let mut x = start;
            while x != end {
                x += step;
                SendMessageW(slider, WM_MOUSEMOVE, LEFT_BUTTON_DOWN, coords(x));
                UpdateWindow(slider);
            }
            // Compare while still dragging: mouse-up/refresh must not conceal
            // trails that were visible during the gesture.
            let result = verify_slider_repaint(a, slider, &format!("{tag}-{direction}"));
            SendMessageW(slider, WM_LBUTTONUP, 0, coords(end));
            result?;
            let value = SendMessageW(slider, TBM_GETPOS, 0, 0) as i32;
            if value == 100 || a.settings.chibi_body_scale as i32 != value {
                return Err("拖动未更新 Q 版身体大小".into());
            }
            let mut label = [0u16; 32];
            let len = GetWindowTextW(a.ctrl(612), label.as_mut_ptr(), label.len() as i32);
            if String::from_utf16_lossy(&label[..len as usize]) != format!("{value}%") {
                return Err("Q 版身体大小标签未同步".into());
            }
        }
        for _ in 0..30 {
            SendMessageW(slider, WM_KEYDOWN, VK_RIGHT as usize, 0);
            UpdateWindow(slider);
        }
        verify_slider_repaint(a, slider, &format!("{tag}-keyboard"))?;
    }
    a.settings.chibi = original.0;
    a.settings.chibi_body_scale = original.1;
    a.refresh_panel();
    Ok(())
}

pub(super) unsafe fn panel_self_test(a: &mut App) -> Result<(), String> {
    if a.panel.is_null() {
        return Err("控制台未创建".into());
    }
    let old_font = a.font;
    // Deliver the system notification locally without modifying Windows settings.
    SendMessageW(
        a.panel,
        WM_SETTINGCHANGE,
        SPI_SETNONCLIENTMETRICS as usize,
        0,
    );
    if a.font.is_null()
        || a.font == old_font
        || SendMessageW(a.ctrl(139), WM_GETFONT, 0, 0) != a.font as isize
    {
        return Err("系统字体通知后控件未更新字体".into());
    }
    let mut font_spec: LOGFONTW = std::mem::zeroed();
    GetObjectW(
        a.font,
        std::mem::size_of::<LOGFONTW>() as i32,
        &mut font_spec as *mut _ as *mut _,
    );
    let font_name = String::from_utf16_lossy(
        &font_spec.lfFaceName[..font_spec
            .lfFaceName
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(font_spec.lfFaceName.len())],
    );
    let pet_size = [a.width, a.height];
    let mut observations = Vec::new();
    for dark in [false, true] {
        a.theme_override = Some(dark);
        a.apply_theme();
        a.panel_ui.motion_enabled = false;
        a.panel_ui.keyboard = true;
        let frame = a.panel_frame(GetDpiForWindow(a.panel));
        SetWindowPos(
            a.panel,
            null_mut(),
            0,
            0,
            (PANEL_WIDTH * a.panel_scale).round() as i32 + frame[0],
            (PANEL_HEIGHT * a.panel_scale).round() as i32 + frame[1],
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        for page in [0, 3, 4, 1, 2] {
            SendMessageW(a.ctrl(150 + page), BM_CLICK, 0, 0);
            if a.panel_page != page as u8 {
                return Err(format!("导航控件没有切到第 {page} 页"));
            }
            a.refresh_panel();
            RedrawWindow(
                a.panel,
                null(),
                null_mut(),
                RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_UPDATENOW,
            );
            save_panel_image(
                a,
                &format!(
                    "console-{}-page-{page}.png",
                    if dark { "dark" } else { "light" }
                ),
            )?;
            let mut visible_count = 0;
            for &(hwnd, owner) in &a.page_controls {
                let visible = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_VISIBLE != 0;
                if visible != (owner == GLOBAL_PAGE || owner == page as u8) {
                    return Err("页面控件可见性错误".into());
                }
                visible_count += visible as usize;
            }
            observations.push(serde_json::json!({"theme":if dark {"dark"}else{"light"},"page":page,"visibleControls":visible_count}));
        }
    }
    slider_drag_self_test(a)?;
    a.show_page(0);
    let original_chibi = a.settings.chibi;
    SendMessageW(a.ctrl(139), BM_CLICK, 0, 0);
    if a.settings.chibi == original_chibi {
        return Err("Q 版开关未更新业务设置".into());
    }
    if (IsWindowEnabled(a.ctrl(212)) != 0) != a.settings.chibi {
        return Err("Q 版滑块禁用状态错误".into());
    }
    SendMessageW(a.ctrl(139), BM_CLICK, 0, 0);
    a.show_page(4);
    let squash = a.settings.typing_squash;
    SendMessageW(a.ctrl(211), TBM_SETPOS, 1, 12);
    SendMessageW(
        a.panel,
        WM_HSCROLL,
        TB_ENDTRACK as usize,
        a.ctrl(211) as isize,
    );
    if (a.settings.typing_squash - 12.).abs() > 0.01 {
        return Err("滑块未更新业务设置".into());
    }
    SendMessageW(a.ctrl(211), TBM_SETPOS, 1, squash.round() as isize);
    SendMessageW(
        a.panel,
        WM_HSCROLL,
        TB_ENDTRACK as usize,
        a.ctrl(211) as isize,
    );
    let dpi = GetDpiForWindow(a.panel).max(96);
    let frame = a.panel_frame(dpi);
    let minimum = a.panel_default_scale(MonitorFromWindow(a.panel, MONITOR_DEFAULTTONEAREST), dpi);
    let base = [
        (PANEL_WIDTH * minimum).round() as i32,
        (PANEL_HEIGHT * minimum).round() as i32,
    ];
    let mut monitor: MONITORINFO = std::mem::zeroed();
    monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(
        MonitorFromWindow(a.panel, MONITOR_DEFAULTTONEAREST),
        &mut monitor,
    );
    let extra_width =
        260.min((monitor.rcWork.right - monitor.rcWork.left - frame[0] - base[0]).max(0));
    let extra_height =
        16.min((monitor.rcWork.bottom - monitor.rcWork.top - frame[1] - base[1]).max(0));
    if extra_width == 0 || extra_height == 0 {
        return Err("显示器没有空间验证独立宽高调整".into());
    }
    for (name, extra) in [
        ("minimum", [0, 0]),
        ("wide", [extra_width, 0]),
        ("tall", [0, extra_height]),
        ("large", [extra_width, extra_height]),
    ] {
        SetWindowPos(
            a.panel,
            null_mut(),
            0,
            0,
            base[0] + extra[0] + frame[0],
            base[1] + extra[1] + frame[1],
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(a.panel, &mut client);
        if client.right != base[0] + extra[0] || client.bottom != base[1] + extra[1] {
            return Err(format!(
                "独立宽高调整错误: {name} expected={}x{} actual={}x{}",
                base[0] + extra[0],
                base[1] + extra[1],
                client.right,
                client.bottom
            ));
        }
        for page in [0, 3, 4, 1, 2] {
            a.show_page(page);
            let mut visible_bounds: Vec<(HWND, RECT)> = Vec::new();
            for &(hwnd, _) in &a.layout {
                if GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_VISIBLE == 0 {
                    continue;
                }
                let mut actual: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut actual);
                MapWindowPoints(
                    null_mut(),
                    a.panel,
                    &mut actual as *mut RECT as *mut POINT,
                    2,
                );
                if actual.left < 0
                    || actual.top < 0
                    || actual.right > client.right + 1
                    || actual.bottom > client.bottom + 1
                {
                    return Err(format!("{name} 页面{page}控件{}越界", GetDlgCtrlID(hwnd)));
                }
                for &(other, bounds) in &visible_bounds {
                    if actual.left < bounds.right
                        && actual.right > bounds.left
                        && actual.top < bounds.bottom
                        && actual.bottom > bounds.top
                    {
                        return Err(format!(
                            "{name} 页面{page}控件{}与{}重叠",
                            GetDlgCtrlID(hwnd),
                            GetDlgCtrlID(other)
                        ));
                    }
                }
                visible_bounds.push((hwnd, actual));
            }
        }
        save_panel_image(a, &format!("console-size-{name}.png"))?;
        observations.push(serde_json::json!({"size":name,"client":[client.right,client.bottom]}));
    }
    SetWindowPos(
        a.panel,
        null_mut(),
        0,
        0,
        100,
        100,
        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
    let mut actual: RECT = std::mem::zeroed();
    GetClientRect(a.panel, &mut actual);
    if actual.right < base[0] || actual.bottom < base[1] {
        return Err("最小尺寸约束失效".into());
    }
    ShowWindow(a.panel, SW_MAXIMIZE);
    GetClientRect(a.panel, &mut actual);
    observations.push(serde_json::json!({"maximizedClient":[actual.right,actual.bottom]}));
    ShowWindow(a.panel, SW_RESTORE);
    // Stress the real navigation command path; no transient overlay windows
    // may survive because pages are committed in one composited repaint.
    let navigation_started = Instant::now();
    for n in 0..100 {
        SendMessageW(a.panel, WM_COMMAND, (150 + n % 5) as usize, 0);
    }
    observations
        .push(serde_json::json!({"navigation100Ms":navigation_started.elapsed().as_millis()}));
    if FindWindowExW(
        a.panel,
        null_mut(),
        wide("MinecraftConsoleTransition").as_ptr(),
        null(),
    ) != null_mut()
    {
        return Err("发现残留的切页叠加窗口".into());
    }
    crate::tray_panel::self_test(a)?;
    if [a.width, a.height] != pet_size {
        return Err("控制台缩放意外改变桌宠本体大小".into());
    }
    config::atomic(
        &a.store.dir.join("ui-self-test.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "passed":true,"navigation":true,"switches":true,"sliderPersistence":true,
            "petSizeUnchanged":true,"atomicNavigation":true,"trayPanel":true,
            "trayPresentedPixels":true,"trayTabStressClicks":80,"layoutOverlapChecked":true,
            "sliderDragPresentedPixels":true,"sliderKeyboardPresentedPixels":true,
            "systemFont":{"face":font_name,"pixelHeight":font_spec.lfHeight,"settingsNotification":true},
            "defaultLogicalSize":[PANEL_WIDTH,PANEL_HEIGHT],
            "independentResize":true,"minimumSize":true,"observations":observations
        }))
        .map_err(|e| e.to_string())?
        .as_slice(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod console_geometry_tests {
    use super::*;
    #[test]
    fn default_minimum_fits_work_area_at_multiple_dpis() {
        for dpi in [96, 120, 144, 192] {
            for work in [[1280, 680], [1920, 1032], [2560, 1392]] {
                let frame = [16 * dpi as i32 / 96, 39 * dpi as i32 / 96];
                let s = default_panel_scale(dpi, work, frame);
                assert!(PANEL_WIDTH * s + frame[0] as f32 + 23. <= work[0] as f32);
                assert!(PANEL_HEIGHT * s + frame[1] as f32 + 23. <= work[1] as f32);
            }
        }
    }
    #[test]
    fn regular_display_uses_the_declared_default() {
        assert_eq!(default_panel_scale(96, [1920, 1080], [16, 39]), 1.);
        assert_eq!(default_panel_scale(144, [2560, 1440], [24, 58]), 1.5);
    }
}
pub(super) unsafe fn diagnostic_snapshot(a: &mut App) {
    if !a.panel.is_null() {
        let _ = save_panel_image(a, "console.png");
    }
    let _ = config::atomic(
        &a.store.dir.join("input-trace.json"),
        serde_json::to_string_pretty(&a.input_trace)
            .unwrap()
            .as_bytes(),
    );
    let _ = config::atomic(
        &a.store.dir.join("appearance.json"),
        serde_json::to_vec(&serde_json::json!({"opacity":a.opacity}))
            .unwrap()
            .as_slice(),
    );
    let controls: Vec<_> = a
        .controls
        .iter()
        .map(|(id, h)| serde_json::json!({"id":id,"hwnd":*h as usize}))
        .collect();
    let mut panel_client: RECT = std::mem::zeroed();
    if !a.panel.is_null() {
        GetClientRect(a.panel, &mut panel_client);
    }
    let _ = config::atomic(
        &a.store.dir.join("console-geometry.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "client": [panel_client.right,panel_client.bottom],
            "scale": a.panel_scale, "defaultLogicalSize": [PANEL_WIDTH,PANEL_HEIGHT], "aspectLocked": false,
             "dpi": a.panel_ui.dpi,
            "page": a.panel_page, "motionEnabled": a.panel_ui.motion_enabled
        }))
        .unwrap()
        .as_slice(),
    );
    let data = serde_json::json!({"pid":std::process::id(),"pet":a.hwnd as usize,"panel":a.panel as usize,"controls":controls,"settings":a.settings,"page":a.panel_page,"renderCount":a.render_count,"musicPlaying":a.pose.music_playing,"interaction":a.pose.interaction_state(),"darkMode":a.panel_dark,"effects":a.pose.effect_counts(),"rasterUs":a.raster_us,"presentUs":a.present_us,"tickGapMaxUs":a.tick_gap_max_us,"tickGapAvgUs":a.tick_gap_sum_us/a.tick_gap_count.max(1),"clicks":a.click_count,"dpi":a.dpi,"head":{"yaw":a.pose.yaw,"pitch":a.pose.pitch,"local":a.pose.head_rotation.to_array()},"sit":a.pose.sit,"crouch":a.pose.crouch,"arms":a.pose.arms.map(|q|q.to_array()),"hook":!a.hook.is_null(),"clickPolling":true,"trayOk":a.tray_ok,"trayError":a.tray_error,"heldKeys":a.pose.held_codes(),"squash":a.pose.squash,"swingProgress":a.pose.swing_progress,"bodyYaw":a.pose.body_yaw,"onlinePending":a.download.is_some(),"onlineNotice":a.online_notice,"bounds":a.frame.bounds,"position":[a.pos.x,a.pos.y],"size":[a.width,a.height]});
    let _ = config::atomic(
        &a.store.dir.join("diagnostic.json"),
        serde_json::to_string_pretty(&data).unwrap().as_bytes(),
    );
    let _ = image::save_buffer(
        a.store.dir.join("pet.png"),
        &a.frame.rgba(),
        a.frame.width as u32,
        a.frame.height as u32,
        image::ColorType::Rgba8,
    );
}
