use super::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use windows_sys::Win32::System::Registry::*;

pub(super) const ANIMATION_TIMER: usize = 0xCC01;
#[derive(Clone, Copy, Default, PartialEq)]
pub(super) enum TextRole {
    #[default]
    Body,
    Caption,
    Section,
    Title,
}
pub(super) struct Card {
    pub page: u8,
    pub bounds: [i32; 4],
    pub separators: Vec<i32>,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Motion {
    from: f32,
    to: f32,
    started: f64,
    duration: f64,
}
impl Motion {
    pub fn value(self, now: f64) -> f32 {
        if self.duration <= 0. {
            return self.to;
        }
        let progress = ((now - self.started) / self.duration).clamp(0., 1.) as f32;
        self.from + (self.to - self.from) * ease_out(progress)
    }
    pub fn active(self, now: f64) -> bool {
        self.from != self.to && now < self.started + self.duration
    }
    fn target(&mut self, to: f32, now: f64, duration: f64) {
        if self.to == to {
            return;
        }
        self.from = self.value(now);
        self.to = to;
        self.started = now;
        self.duration = duration;
        if duration == 0. {
            self.from = to;
        }
    }
}
// Solve the x axis of cubic-bezier(0.23,1,0.32,1), so interruptions
// continue at the visible value instead of restarting from an endpoint.
fn ease_out(x: f32) -> f32 {
    if x <= 0. || x >= 1. {
        return x.clamp(0., 1.);
    }
    let (mut lo, mut hi) = (0f32, 1f32);
    for _ in 0..16 {
        let t = (lo + hi) * 0.5;
        let u = 1. - t;
        let bx = 3. * u * u * t * 0.23 + 3. * u * t * t * 0.32 + t * t * t;
        if bx < x {
            lo = t;
        } else {
            hi = t;
        }
    }
    let t = (lo + hi) * 0.5;
    1. - (1. - t).powi(3)
}
#[derive(Default)]
pub(super) struct ButtonMotion {
    checked: Motion,
    hover: Motion,
    press: Motion,
}
impl ButtonMotion {
    fn active(&self, now: f64) -> bool {
        self.checked.active(now) || self.hover.active(now) || self.press.active(now)
    }
}
pub(super) struct PanelUi {
    pub cards: Vec<Card>,
    pub roles: HashMap<usize, TextRole>,
    pub buttons: HashMap<usize, ButtonMotion>,
    pub keyboard: bool,
    pub motion_enabled: bool,
    pub client: [i32; 2],
    pub dpi: u32,
    fonts: [HFONT; 3],
    font_scale: f32,
    cache: RefCell<HashMap<usize, (u64, Surface)>>,
}
impl Default for PanelUi {
    fn default() -> Self {
        Self {
            cards: Vec::new(),
            roles: HashMap::new(),
            buttons: HashMap::new(),
            keyboard: false,
            motion_enabled: true,
            client: [ui::PANEL_WIDTH as i32, ui::PANEL_HEIGHT as i32],
            dpi: 96,
            fonts: [null_mut(); 3],
            font_scale: 0.,
            cache: RefCell::new(HashMap::new()),
        }
    }
}
impl PanelUi {
    pub fn clear_controls(&mut self) {
        self.cards.clear();
        self.roles.clear();
        self.buttons.clear();
        self.client = [ui::PANEL_WIDTH as i32, ui::PANEL_HEIGHT as i32];
        self.keyboard = false;
        self.cache.get_mut().clear();
    }
}
impl Drop for PanelUi {
    fn drop(&mut self) {
        unsafe {
            for font in self.fonts {
                if !font.is_null() {
                    DeleteObject(font);
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Palette {
    app: u32,
    sidebar: u32,
    card: u32,
    subtle: u32,
    selected: u32,
    border: u32,
    strong: u32,
    text: u32,
    muted: u32,
    caption: u32,
    action: u32,
    action_hover: u32,
    action_soft: u32,
    action_text: u32,
    off: u32,
    success: u32,
}
pub(super) const fn rgb(hex: u32) -> u32 {
    ((hex & 255) << 16) | (hex & 0xff00) | ((hex >> 16) & 255)
}
impl Palette {
    fn new(dark: bool) -> Self {
        // Ported from farion1231/cc-switch v7 design tokens (MIT); see notices.
        if dark {
            Self {
                app: rgb(0x1c1c1e),
                sidebar: rgb(0x161618),
                card: rgb(0x2e2e33),
                subtle: rgb(0x26262a),
                selected: rgb(0x313136),
                border: rgb(0x3e3e45),
                strong: rgb(0x4a4a52),
                text: rgb(0xe4e4e7),
                muted: rgb(0xababb3),
                caption: rgb(0x9a9aa2),
                action: rgb(0xea580c),
                action_hover: rgb(0xf06d1f),
                action_soft: rgb(0x472e23),
                action_text: rgb(0xfb923c),
                off: rgb(0x6f6f77),
                success: rgb(0x4ade80),
            }
        } else {
            Self {
                app: rgb(0xffffff),
                sidebar: rgb(0xf5f5f7),
                card: rgb(0xffffff),
                subtle: rgb(0xefeff2),
                selected: rgb(0xe4e4ea),
                border: rgb(0xe4e4e9),
                strong: rgb(0xcfcfd6),
                text: rgb(0x18181b),
                muted: rgb(0x52525b),
                caption: rgb(0x65656e),
                action: rgb(0xf97316),
                action_hover: rgb(0xea620a),
                action_soft: rgb(0xfff1e6),
                action_text: rgb(0xc2410c),
                off: rgb(0x888890),
                success: rgb(0x16a34a),
            }
        }
    }
}
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0., 1.);
    let mut color = 0;
    for shift in [0, 8, 16] {
        let from = ((a >> shift) & 255) as f32;
        let to = ((b >> shift) & 255) as f32;
        color |= ((from + (to - from) * t).round() as u32) << shift;
    }
    color
}
pub(super) unsafe fn system_dark() -> bool {
    let mut value = 1u32;
    let mut len = 4u32;
    RegGetValueW(
        HKEY_CURRENT_USER,
        wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize").as_ptr(),
        wide("AppsUseLightTheme").as_ptr(),
        RRF_RT_REG_DWORD,
        null_mut(),
        &mut value as *mut _ as *mut _,
        &mut len,
    ) == 0
        && value == 0
}
// Rasterize vector edges at 3x resolution and downsample once. Native edit
// fields retain system rendering; grayscale text AA avoids RGB fringes.
pub(super) unsafe fn smooth_paint(dc: HDC, r: RECT, paint: impl FnOnce(HDC)) {
    let width = r.right - r.left;
    let height = r.bottom - r.top;
    if width <= 0 || height <= 0 {
        return;
    }
    let surface = Surface::new(width * 3, height * 3);
    if surface.bits.is_null() {
        paint(dc);
        return;
    }
    SetMapMode(surface.dc, MM_ANISOTROPIC);
    SetWindowExtEx(surface.dc, width, height, null_mut());
    SetViewportExtEx(surface.dc, width * 3, height * 3, null_mut());
    SetWindowOrgEx(surface.dc, r.left, r.top, null_mut());
    paint(surface.dc);
    SetMapMode(surface.dc, MM_TEXT);
    SetWindowOrgEx(surface.dc, 0, 0, null_mut());
    SetViewportOrgEx(surface.dc, 0, 0, null_mut());
    let state = SaveDC(dc);
    SetStretchBltMode(dc, HALFTONE);
    SetBrushOrgEx(dc, 0, 0, null_mut());
    StretchBlt(
        dc,
        r.left,
        r.top,
        width,
        height,
        surface.dc,
        0,
        0,
        width * 3,
        height * 3,
        SRCCOPY,
    );
    RestoreDC(dc, state);
}
pub(super) unsafe fn fill(dc: HDC, rect: &RECT, color: u32) {
    SetDCBrushColor(dc, color);
    FillRect(dc, rect, GetStockObject(DC_BRUSH) as HBRUSH);
}
pub(super) unsafe fn rounded(
    dc: HDC,
    rect: RECT,
    radius: i32,
    color: u32,
    outline: Option<(u32, i32)>,
) {
    let brush = SelectObject(dc, GetStockObject(DC_BRUSH));
    SetDCBrushColor(dc, color);
    let pen = outline.map(|(color, width)| CreatePen(PS_SOLID, width.max(1), color));
    let oldpen = SelectObject(dc, pen.unwrap_or(GetStockObject(NULL_PEN)));
    RoundRect(
        dc,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        radius * 2,
        radius * 2,
    );
    SelectObject(dc, oldpen);
    SelectObject(dc, brush);
    if let Some(pen) = pen {
        DeleteObject(pen);
    }
}
pub(super) unsafe fn line(dc: HDC, x1: i32, y1: i32, x2: i32, y2: i32, color: u32, width: i32) {
    let pen = CreatePen(PS_SOLID, width.max(1), color);
    let old = SelectObject(dc, pen);
    MoveToEx(dc, x1, y1, null_mut());
    LineTo(dc, x2, y2);
    SelectObject(dc, old);
    DeleteObject(pen);
}
pub(super) unsafe fn text(
    dc: HDC,
    content: &str,
    mut rect: RECT,
    font: HFONT,
    color: u32,
    align: u32,
) {
    let old = SelectObject(dc, font);
    SetBkMode(dc, TRANSPARENT as i32);
    SetTextColor(dc, color);
    let chars = wide(content);
    DrawTextW(
        dc,
        chars.as_ptr(),
        (chars.len() - 1) as i32,
        &mut rect,
        DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX | align,
    );
    SelectObject(dc, old);
}
unsafe fn system_ui_font() -> LOGFONTW {
    let mut metrics: NONCLIENTMETRICSW = std::mem::zeroed();
    metrics.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
    // Query at 96 DPI: the caller applies its own monitor's scale once.
    if SystemParametersInfoForDpi(
        SPI_GETNONCLIENTMETRICS,
        metrics.cbSize,
        &mut metrics as *mut _ as *mut _,
        0,
        96,
    ) != 0
    {
        return metrics.lfMessageFont;
    }
    let mut fallback: LOGFONTW = std::mem::zeroed();
    GetObjectW(
        GetStockObject(DEFAULT_GUI_FONT),
        std::mem::size_of::<LOGFONTW>() as i32,
        &mut fallback as *mut _ as *mut _,
    );
    fallback
}

pub(super) unsafe fn font(size: f32, weight: i32) -> HFONT {
    let mut spec = system_ui_font();
    let scale = size.max(1.) / 13.;
    let height = if spec.lfHeight == 0 {
        -12
    } else {
        spec.lfHeight
    };
    spec.lfHeight = (height as f32 * scale).round() as i32;
    spec.lfWidth = (spec.lfWidth as f32 * scale).round() as i32;
    // Preserve the user's regular system weight; headings retain emphasis.
    if weight != FW_NORMAL as i32 {
        spec.lfWeight = weight;
    }
    spec.lfQuality = ANTIALIASED_QUALITY;
    CreateFontIndirectW(&spec)
}

impl App {
    unsafe fn cached_paint(
        &self,
        slot: usize,
        signature: impl Hash,
        dc: HDC,
        r: RECT,
        paint: impl FnOnce(HDC),
    ) {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        signature.hash(&mut hash);
        let signature = hash.finish();
        let width = r.right - r.left;
        let height = r.bottom - r.top;
        if width <= 0 || height <= 0 {
            return;
        }
        let valid = self
            .panel_ui
            .cache
            .borrow()
            .get(&slot)
            .is_some_and(|(key, image)| *key == signature && image.w == width && image.h == height);
        if !valid {
            let surface = Surface::new(width, height);
            if surface.bits.is_null() {
                smooth_paint(dc, r, paint);
                return;
            }
            SetViewportOrgEx(surface.dc, -r.left, -r.top, null_mut());
            smooth_paint(surface.dc, r, paint);
            SetViewportOrgEx(surface.dc, 0, 0, null_mut());
            self.panel_ui
                .cache
                .borrow_mut()
                .insert(slot, (signature, surface));
        }
        if let Some((_, surface)) = self.panel_ui.cache.borrow().get(&slot) {
            BitBlt(dc, r.left, r.top, width, height, surface.dc, 0, 0, SRCCOPY);
        }
    }
    pub(super) fn panel_rect(&self, [x, y, w, h]: [i32; 4]) -> RECT {
        let s = self.panel_scale.max(0.01);
        let extra_w = (self.panel_ui.client[0] as f32 / s - ui::PANEL_WIDTH).max(0.);
        let extra_h = (self.panel_ui.client[1] as f32 / s - ui::PANEL_HEIGHT).max(0.);
        let map_x = |v: i32| {
            let x = v as f32;
            if v >= 736 {
                x + extra_w
            } else if v >= 192 {
                x + (x - 192.) / 544. * extra_w
            } else {
                x
            }
        };
        // Anchor complete footer controls, not individual rectangle edges;
        // otherwise a button crossing the footer line stretches on resize.
        let bottom_anchored = y >= 646 || (x < 168 && y >= 582);
        let map_y = |v: i32| {
            v as f32
                + if bottom_anchored || v >= ui::PANEL_HEIGHT as i32 {
                    extra_h
                } else {
                    0.
                }
        };
        RECT {
            left: (map_x(x) * s).round() as i32,
            top: (map_y(y) * s).round() as i32,
            right: (map_x(x + w) * s).round() as i32,
            bottom: (map_y(y + h) * s).round() as i32,
        }
    }
    fn panel_font(&self, role: TextRole) -> HFONT {
        match role {
            TextRole::Body => self.font,
            TextRole::Caption => self.panel_ui.fonts[0],
            TextRole::Section => self.panel_ui.fonts[1],
            TextRole::Title => self.panel_ui.fonts[2],
        }
    }
    fn control_background(&self, hwnd: HWND) -> u32 {
        let p = Palette::new(self.panel_dark);
        if let Some((_, [x, y, _, _])) = self.layout.iter().find(|(h, _)| *h == hwnd) {
            if *x < 168 {
                return p.sidebar;
            }
            if self.panel_ui.cards.iter().any(|c| {
                c.page == self.panel_page
                    && *x >= c.bounds[0]
                    && *y >= c.bounds[1]
                    && *y < c.bounds[1] + c.bounds[3]
            }) {
                return p.card;
            }
        }
        p.app
    }
    pub(super) unsafe fn rebuild_panel_fonts(&mut self) {
        let s = self.panel_scale;
        let rebuild = self.font.is_null()
            || [12., 13., 15., 18.]
                .iter()
                .any(|size| (size * s).round() != (size * self.panel_ui.font_scale).round());
        let old_font = self.font;
        let old_extra = self.panel_ui.fonts;
        if rebuild {
            self.panel_ui.cache.get_mut().clear();
            self.font = font(13. * s, 400);
            self.panel_ui.fonts = [font(12. * s, 400), font(15. * s, 600), font(18. * s, 600)];
            self.panel_ui.font_scale = s;
        }
        for &(hwnd, _) in &self.layout {
            let role = self
                .panel_ui
                .roles
                .get(&(hwnd as usize))
                .copied()
                .unwrap_or_default();
            SendMessageW(hwnd, WM_SETFONT, self.panel_font(role) as usize, 0);
        }
        if rebuild {
            if !old_font.is_null() {
                DeleteObject(old_font);
            }
            for old in old_extra {
                if !old.is_null() {
                    DeleteObject(old);
                }
            }
        }
    }
    pub(super) unsafe fn apply_theme(&mut self) {
        self.panel_dark = self.theme_override.unwrap_or_else(|| system_dark());
        let mut animations = 1i32;
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            &mut animations as *mut _ as *mut _,
            0,
        );
        self.panel_ui.motion_enabled = animations != 0;
        if self.panel.is_null() {
            return;
        }
        let dark = self.panel_dark as i32;
        windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            self.panel,
            20,
            &dark as *const _ as *const _,
            4,
        );
        let corner = 2i32;
        windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            self.panel,
            33,
            &corner as *const _ as *const _,
            4,
        );
        self.github_icon();
        RedrawWindow(
            self.panel,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN,
        );
    }
    pub(super) unsafe fn paint_panel(&self, dc: HDC) {
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(self.panel, &mut r);
        let edit_focused = self
            .layout
            .iter()
            .any(|(hwnd, _)| GetDlgCtrlID(*hwnd) == 330 && GetFocus() == *hwnd);
        self.cached_paint(
            usize::MAX - self.panel_page as usize,
            (self.panel_dark, self.panel_scale.to_bits(), edit_focused),
            dc,
            r,
            |buffer| self.paint_panel_content(buffer),
        );
    }
    unsafe fn paint_panel_content(&self, dc: HDC) {
        let p = Palette::new(self.panel_dark);
        let s = self.panel_scale;
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(self.panel, &mut client);
        fill(dc, &client, p.app);
        fill(
            dc,
            &self.panel_rect([0, 0, 168, ui::PANEL_HEIGHT as i32]),
            p.sidebar,
        );
        fill(
            dc,
            &self.panel_rect([167, 0, 1, ui::PANEL_HEIGHT as i32]),
            p.border,
        );
        DrawIconEx(
            dc,
            self.panel_rect([16, 20, 0, 0]).left,
            self.panel_rect([16, 20, 0, 0]).top,
            self.icon,
            (24. * s).round() as i32,
            (24. * s).round() as i32,
            0,
            null_mut(),
            DI_NORMAL,
        );
        text(
            dc,
            "Minecraft",
            self.panel_rect([48, 16, 112, 24]),
            self.panel_font(TextRole::Section),
            p.text,
            DT_LEFT,
        );
        text(
            dc,
            "桌宠控制台",
            self.panel_rect([48, 40, 112, 20]),
            self.panel_font(TextRole::Caption),
            p.caption,
            DT_LEFT,
        );
        text(
            dc,
            "偏好设置",
            self.panel_rect([20, 80, 128, 20]),
            self.panel_font(TextRole::Caption),
            p.caption,
            DT_LEFT,
        );
        text(
            dc,
            &format!(
                "Windows · v{}",
                VERSION.strip_suffix(".0").unwrap_or(VERSION)
            ),
            self.panel_rect([20, 666, 136, 20]),
            self.panel_font(TextRole::Caption),
            p.caption,
            DT_LEFT,
        );
        let (title, subtitle) = match self.panel_page {
            1 => ("视线与角度", "调整角色朝向和鼠标跟随"),
            2 => ("自定义姿态", "为站姿和坐姿设置独立角度"),
            3 => ("交互与动画", "让桌宠跟随你的每一次操作"),
            4 => ("桌面与性能", "管理桌面行为和渲染效果"),
            _ => ("外观与皮肤", "打造属于你的桌面伙伴"),
        };
        text(
            dc,
            title,
            self.panel_rect([192, 20, 420, 28]),
            self.panel_font(TextRole::Title),
            p.text,
            DT_LEFT,
        );
        text(
            dc,
            subtitle,
            self.panel_rect([192, 52, 510, 22]),
            self.panel_font(TextRole::Caption),
            p.caption,
            DT_LEFT,
        );
        let badge = self.panel_rect([664, 24, 72, 24]);
        rounded(dc, badge, (6. * s).round() as i32, p.subtle, None);
        text(
            dc,
            "即时生效",
            badge,
            self.panel_font(TextRole::Caption),
            p.muted,
            DT_CENTER,
        );
        for card in self
            .panel_ui
            .cards
            .iter()
            .filter(|card| card.page == self.panel_page)
        {
            let r = self.panel_rect(card.bounds);
            rounded(
                dc,
                r,
                (10. * s).round() as i32,
                p.card,
                Some((p.border, s.round() as i32)),
            );
            for y in &card.separators {
                fill(
                    dc,
                    &self.panel_rect([
                        card.bounds[0] + 1,
                        card.bounds[1] + y,
                        card.bounds[2] - 2,
                        1,
                    ]),
                    p.border,
                );
            }
        }
        for &(hwnd, bounds) in &self.layout {
            if GetDlgCtrlID(hwnd) == 330 && self.panel_page == 0 {
                let [x, y, w, h] = bounds;
                let focus = GetFocus() == hwnd;
                rounded(
                    dc,
                    self.panel_rect([x - 7, y - 5, w + 14, h + 10]),
                    (6. * s).round() as i32,
                    p.card,
                    Some((
                        if focus { p.action } else { p.strong },
                        (if focus { 2. * s } else { s }).round() as i32,
                    )),
                );
            }
        }
        fill(dc, &self.panel_rect([192, 646, 544, 1]), p.border);
        rounded(
            dc,
            self.panel_rect([194, 670, 7, 7]),
            (4. * s).round() as i32,
            p.success,
            None,
        );
        let grip = self.panel_rect([742, 682, 12, 12]);
        for n in [4, 8, 12] {
            let n = (n as f32 * s).round() as i32;
            line(
                dc,
                grip.right - n,
                grip.bottom,
                grip.right,
                grip.bottom - n,
                p.strong,
                s.round() as i32,
            );
        }
    }
    pub(super) unsafe fn theme_message(
        &mut self,
        hwnd: HWND,
        msg: u32,
        w: WPARAM,
        l: LPARAM,
    ) -> Option<LRESULT> {
        match msg {
            WM_SETTINGCHANGE | WM_THEMECHANGED => {
                self.panel_ui.font_scale = 0.;
                self.rebuild_panel_fonts();
                self.apply_theme();
                Some(0)
            }
            WM_ERASEBKGND => Some(1),
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(hwnd, &mut ps);
                let mut rc: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rc);
                if rc.right > 0 && rc.bottom > 0 {
                    let surface = Surface::new(rc.right, rc.bottom);
                    if !surface.bits.is_null() {
                        self.paint_panel(surface.dc);
                        BitBlt(dc, 0, 0, rc.right, rc.bottom, surface.dc, 0, 0, SRCCOPY);
                    }
                }
                EndPaint(hwnd, &ps);
                Some(0)
            }
            WM_PRINTCLIENT => {
                self.paint_panel(w as HDC);
                Some(0)
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                let dc = w as HDC;
                let child = l as HWND;
                let p = Palette::new(self.panel_dark);
                let background = self.control_background(child);
                let role = self
                    .panel_ui
                    .roles
                    .get(&(child as usize))
                    .copied()
                    .unwrap_or_default();
                let color = if role == TextRole::Caption {
                    p.caption
                } else {
                    p.text
                };
                SetTextColor(
                    dc,
                    if IsWindowEnabled(child) != 0 {
                        color
                    } else {
                        mix(background, color, 0.45)
                    },
                );
                SetBkColor(dc, background);
                SetBkMode(dc, TRANSPARENT as i32);
                SetDCBrushColor(dc, background);
                Some(GetStockObject(DC_BRUSH) as isize)
            }
            WM_NOTIFY => self.draw_control(l),
            _ => None,
        }
    }
    unsafe fn draw_control(&self, l: LPARAM) -> Option<LRESULT> {
        if l == 0 {
            return None;
        }
        let hdr = &*(l as *const NMHDR);
        if hdr.code != NM_CUSTOMDRAW {
            return None;
        }
        let draw = &*(l as *const NMCUSTOMDRAW);
        if draw.dwDrawStage != CDDS_PREPAINT {
            return None;
        }
        let hwnd = hdr.hwndFrom;
        let mut class = [0u16; 64];
        let n = GetClassNameW(hwnd, class.as_mut_ptr(), 64);
        match String::from_utf16_lossy(&class[..n as usize]).as_ref() {
            "Button" => {
                let now = self.start.elapsed().as_secs_f64();
                let motion = self.panel_ui.buttons.get(&(hwnd as usize)).map(|m| {
                    (
                        m.checked.value(now).to_bits(),
                        m.hover.value(now).to_bits(),
                        m.press.value(now).to_bits(),
                    )
                });
                let mut label = vec![0u16; GetWindowTextLengthW(hwnd) as usize + 1];
                GetWindowTextW(hwnd, label.as_mut_ptr(), label.len() as i32);
                let key = (
                    self.panel_dark,
                    self.panel_scale.to_bits(),
                    IsWindowEnabled(hwnd),
                    draw.uItemState,
                    GetFocus() == hwnd,
                    SendMessageW(hwnd, WM_QUERYUISTATE, 0, 0),
                    motion,
                    label,
                    self.control_background(hwnd),
                );
                self.cached_paint(hwnd as usize, key, draw.hdc, draw.rc, |dc| {
                    let mut buffered = *draw;
                    buffered.hdc = dc;
                    self.paint_button(hwnd, &buffered);
                });
                Some(CDRF_SKIPDEFAULT as isize)
            }
            "msctls_trackbar32" => {
                // Trackbar PREPAINT does not guarantee a populated rc.
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let mut channel: RECT = std::mem::zeroed();
                let mut thumb: RECT = std::mem::zeroed();
                SendMessageW(hwnd, TBM_GETCHANNELRECT, 0, &mut channel as *mut _ as isize);
                SendMessageW(hwnd, TBM_GETTHUMBRECT, 0, &mut thumb as *mut _ as isize);
                let key = (
                    self.panel_dark,
                    self.panel_scale.to_bits(),
                    IsWindowEnabled(hwnd),
                    GetFocus() == hwnd,
                    SendMessageW(hwnd, WM_QUERYUISTATE, 0, 0),
                    SendMessageW(hwnd, ui::TBM_GETPOS, 0, 0),
                    self.control_background(hwnd),
                    (
                        channel.left,
                        channel.top,
                        channel.right,
                        channel.bottom,
                        thumb.left,
                        thumb.top,
                        thumb.right,
                        thumb.bottom,
                    ),
                );
                self.cached_paint(hwnd as usize, key, draw.hdc, client, |dc| {
                    let mut buffered = *draw;
                    buffered.hdc = dc;
                    buffered.rc = client;
                    self.paint_slider(hwnd, &buffered);
                });
                Some(CDRF_SKIPDEFAULT as isize)
            }
            _ => None,
        }
    }
    unsafe fn paint_button(&self, hwnd: HWND, draw: &NMCUSTOMDRAW) {
        let dc = draw.hdc;
        let p = Palette::new(self.panel_dark);
        let s = self.panel_scale;
        let id = GetDlgCtrlID(hwnd);
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let ty = style & BS_TYPEMASK as u32;
        let check = ty == BS_CHECKBOX as u32;
        let radio = ty == BS_RADIOBUTTON as u32;
        let nav = (150..=154).contains(&id);
        let ghost = nav || id == 169 || id == 170;
        let primary = id == 101;
        let enabled = IsWindowEnabled(hwnd) != 0;
        let now = self.start.elapsed().as_secs_f64();
        let motion = self.panel_ui.buttons.get(&(hwnd as usize));
        let checked = motion.map(|m| m.checked.value(now)).unwrap_or(
            if SendMessageW(hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize {
                1.
            } else {
                0.
            },
        );
        let hover = if enabled {
            motion.map(|m| m.hover.value(now)).unwrap_or(0.)
        } else {
            0.
        };
        let press = if enabled {
            motion.map(|m| m.press.value(now)).unwrap_or(0.)
        } else {
            0.
        };
        let background = self.control_background(hwnd);
        fill(dc, &draw.rc, background);
        let saved = SaveDC(dc);
        if !check && !nav && press > 0. {
            SetGraphicsMode(dc, GM_ADVANCED);
            let k = 1. - press * 0.04;
            let t = XFORM {
                eM11: k,
                eM12: 0.,
                eM21: 0.,
                eM22: k,
                eDx: (draw.rc.right + draw.rc.left) as f32 * (1. - k) * 0.5,
                eDy: (draw.rc.bottom + draw.rc.top) as f32 * (1. - k) * 0.5,
            };
            SetWorldTransform(dc, &t);
        }
        let mut rc = draw.rc;
        InflateRect(
            &mut rc,
            -s.round().max(1.) as i32,
            -s.round().max(1.) as i32,
        );
        let mut fg = p.text;
        if check {
            let width = (30. * s).round() as i32;
            let height = (18. * s).round() as i32;
            let right = rc.right - (2. * s).round() as i32;
            let top = rc.top + (rc.bottom - rc.top - height) / 2;
            let track = RECT {
                left: right - width,
                top,
                right,
                bottom: top + height,
            };
            let track_color = mix(p.off, p.action, checked);
            let track_color = if enabled {
                track_color
            } else {
                mix(background, track_color, 0.45)
            };
            rounded(dc, track, height / 2, track_color, None);
            let knob = (14. * s).round().max(2.) as i32;
            let gap = (2. * s).round() as i32;
            let left =
                track.left + gap + ((width - knob - 2 * gap) as f32 * checked).round() as i32;
            rounded(
                dc,
                RECT {
                    left,
                    top: top + gap,
                    right: left + knob,
                    bottom: top + gap + knob,
                },
                knob / 2,
                if enabled {
                    rgb(0xffffff)
                } else {
                    mix(background, rgb(0xffffff), 0.45)
                },
                None,
            );
            rc.right = track.left - (14. * s).round() as i32;
        } else if ghost {
            let bg = if nav {
                mix(mix(background, p.subtle, hover), p.selected, checked)
            } else {
                mix(background, p.subtle, hover)
            };
            rounded(dc, rc, (6. * s).round() as i32, bg, None);
            fg = if nav {
                mix(p.muted, p.text, checked.max(hover))
            } else {
                mix(p.muted, p.text, hover)
            };
            self.paint_icon(
                dc,
                id,
                RECT {
                    left: rc.left + (10. * s).round() as i32,
                    top: rc.top + (rc.bottom - rc.top - (16. * s).round() as i32) / 2,
                    right: rc.left + (26. * s).round() as i32,
                    bottom: rc.top + (rc.bottom - rc.top + (16. * s).round() as i32) / 2,
                },
                fg,
            );
            rc.left += (36. * s).round() as i32;
        } else {
            let (bg, border) = if primary {
                (mix(p.action, p.action_hover, hover), p.action)
            } else if radio {
                (
                    mix(
                        mix(p.subtle, p.selected, hover * 0.5),
                        p.action_soft,
                        checked,
                    ),
                    mix(p.subtle, p.action, checked),
                )
            } else {
                (mix(p.card, p.subtle, hover), p.strong)
            };
            rounded(
                dc,
                rc,
                (6. * s).round() as i32,
                if enabled {
                    bg
                } else {
                    mix(background, bg, 0.45)
                },
                Some((if enabled { border } else { p.border }, s.round() as i32)),
            );
            if primary {
                fg = rgb(0xffffff);
            } else if radio {
                fg = mix(p.muted, p.action_text, checked);
            }
        }
        if !enabled {
            fg = mix(background, fg, 0.45);
        }
        let mut buffer = [0u16; 256];
        let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), 256);
        text(
            dc,
            &String::from_utf16_lossy(&buffer[..len as usize]),
            rc,
            self.font,
            fg,
            if check || ghost { DT_LEFT } else { DT_CENTER },
        );
        RestoreDC(dc, saved);
        if draw.uItemState & CDIS_FOCUS != 0
            && SendMessageW(hwnd, WM_QUERYUISTATE, 0, 0) & UISF_HIDEFOCUS as isize == 0
        {
            let mut focus = draw.rc;
            InflateRect(&mut focus, -1, -1);
            let pen = CreatePen(PS_SOLID, (2. * s).round().max(1.) as i32, p.action);
            let oldpen = SelectObject(dc, pen);
            let oldbrush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
            RoundRect(
                dc,
                focus.left,
                focus.top,
                focus.right,
                focus.bottom,
                (12. * s).round() as i32,
                (12. * s).round() as i32,
            );
            SelectObject(dc, oldbrush);
            SelectObject(dc, oldpen);
            DeleteObject(pen);
        }
    }
    unsafe fn paint_slider(&self, hwnd: HWND, draw: &NMCUSTOMDRAW) {
        let dc = draw.hdc;
        let p = Palette::new(self.panel_dark);
        let s = self.panel_scale;
        let enabled = IsWindowEnabled(hwnd) != 0;
        let background = self.control_background(hwnd);
        fill(dc, &draw.rc, background);
        let mut channel: RECT = std::mem::zeroed();
        let mut thumb: RECT = std::mem::zeroed();
        SendMessageW(hwnd, TBM_GETCHANNELRECT, 0, &mut channel as *mut _ as isize);
        SendMessageW(hwnd, TBM_GETTHUMBRECT, 0, &mut thumb as *mut _ as isize);
        let cy = (thumb.top + thumb.bottom) / 2;
        let cx = (thumb.left + thumb.right) / 2;
        let thickness = (4. * s).round().max(2.) as i32;
        let track = RECT {
            left: channel.left,
            top: cy - thickness / 2,
            right: channel.right,
            bottom: cy + (thickness + 1) / 2,
        };
        rounded(dc, track, thickness, p.subtle, None);
        let accent = if enabled {
            p.action
        } else {
            mix(background, p.action, 0.45)
        };
        if cx > track.left {
            rounded(dc, RECT { right: cx, ..track }, thickness, accent, None);
        }
        let r = (6. * s).round().max(3.) as i32;
        rounded(
            dc,
            RECT {
                left: cx - r,
                top: cy - r,
                right: cx + r,
                bottom: cy + r,
            },
            r,
            p.card,
            Some((accent, (2. * s).round().max(1.) as i32)),
        );
        if GetFocus() == hwnd
            && SendMessageW(hwnd, WM_QUERYUISTATE, 0, 0) & UISF_HIDEFOCUS as isize == 0
        {
            let mut focus = draw.rc;
            InflateRect(&mut focus, -1, -1);
            DrawFocusRect(dc, &focus);
        }
    }
    unsafe fn paint_icon(&self, dc: HDC, id: i32, rect: RECT, color: u32) {
        let s = self.panel_scale;
        let x = rect.left;
        let y = rect.top;
        let q = |v: f32| (v * s).round() as i32;
        let pen = CreatePen(PS_SOLID, q(1.5).max(1), color);
        let oldpen = SelectObject(dc, pen);
        let oldbrush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
        match id {
            150 => {
                for (xx, yy) in [(0., 0.), (9., 0.), (0., 9.), (9., 9.)] {
                    RoundRect(
                        dc,
                        x + q(xx),
                        y + q(yy),
                        x + q(xx + 6.),
                        y + q(yy + 6.),
                        q(3.),
                        q(3.),
                    );
                }
            }
            151 => {
                Ellipse(dc, x, y + q(3.), x + q(16.), y + q(13.));
                Ellipse(dc, x + q(6.), y + q(6.), x + q(10.), y + q(10.));
            }
            152 => {
                for (yy, xx) in [(3., 5.), (8., 11.), (13., 6.)] {
                    MoveToEx(dc, x, y + q(yy), null_mut());
                    LineTo(dc, x + q(16.), y + q(yy));
                    Ellipse(
                        dc,
                        x + q(xx - 2.),
                        y + q(yy - 2.),
                        x + q(xx + 2.),
                        y + q(yy + 2.),
                    );
                }
            }
            153 => {
                let points = [
                    (8., 0.),
                    (10., 6.),
                    (16., 8.),
                    (10., 10.),
                    (8., 16.),
                    (6., 10.),
                    (0., 8.),
                    (6., 6.),
                    (8., 0.),
                ];
                for (i, (xx, yy)) in points.iter().enumerate() {
                    if i == 0 {
                        MoveToEx(dc, x + q(*xx), y + q(*yy), null_mut());
                    } else {
                        LineTo(dc, x + q(*xx), y + q(*yy));
                    }
                }
            }
            154 => {
                RoundRect(dc, x, y, x + q(16.), y + q(11.), q(3.), q(3.));
                MoveToEx(dc, x + q(8.), y + q(11.), null_mut());
                LineTo(dc, x + q(8.), y + q(15.));
                MoveToEx(dc, x + q(4.), y + q(15.), null_mut());
                LineTo(dc, x + q(12.), y + q(15.));
            }
            169 => {
                if self.github_images != 0 {
                    ImageList_Draw(self.github_images, 0, dc, x - 1, y - 1, ILD_TRANSPARENT);
                }
            }
            170 => {
                Arc(
                    dc,
                    x,
                    y,
                    x + q(16.),
                    y + q(16.),
                    x + q(14.),
                    y + q(5.),
                    x + q(15.),
                    y + q(9.),
                );
                MoveToEx(dc, x + q(15.), y, null_mut());
                LineTo(dc, x + q(15.), y + q(6.));
                LineTo(dc, x + q(9.), y + q(6.));
            }
            _ => {}
        }
        SelectObject(dc, oldbrush);
        SelectObject(dc, oldpen);
        DeleteObject(pen);
    }
    pub(super) unsafe fn capture_panel(&self) -> Option<Surface> {
        let mut rc: RECT = std::mem::zeroed();
        GetClientRect(self.panel, &mut rc);
        if rc.right <= 0 || rc.bottom <= 0 {
            return None;
        }
        let surface = Surface::new(rc.right, rc.bottom);
        if surface.bits.is_null() {
            return None;
        }
        self.paint_panel(surface.dc);
        for &(hwnd, bounds) in &self.layout {
            if GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_VISIBLE == 0 {
                continue;
            }
            let r = self.panel_rect(bounds);
            let state = SaveDC(surface.dc);
            SetViewportOrgEx(surface.dc, r.left, r.top, null_mut());
            IntersectClipRect(surface.dc, 0, 0, r.right - r.left, r.bottom - r.top);
            SendMessageW(
                hwnd,
                WM_PRINT,
                surface.dc as usize,
                (PRF_CLIENT | PRF_ERASEBKGND | PRF_CHILDREN) as isize,
            );
            RestoreDC(surface.dc, state);
        }
        Some(surface)
    }
    pub(super) unsafe fn panel_animation_tick(&mut self) {
        if IsWindowVisible(self.panel) == 0 {
            KillTimer(self.panel, ANIMATION_TIMER);
            return;
        }
        let now = self.start.elapsed().as_secs_f64();
        let mut active = false;
        for (&hwnd, motion) in &self.panel_ui.buttons {
            if motion.active(now - 0.020) {
                InvalidateRect(hwnd as HWND, null(), 0);
            }
            active |= motion.active(now);
        }
        if !active {
            KillTimer(self.panel, ANIMATION_TIMER);
        }
    }
}

pub(super) unsafe extern "system" fn control_proc(
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
    if msg == WM_NCDESTROY {
        (*APP).panel_ui.buttons.remove(&(hwnd as usize));
        RemoveWindowSubclass(hwnd, Some(control_proc), 1);
        return DefSubclassProc(hwnd, msg, w, l);
    }
    let mut class = [0u16; 32];
    let n = GetClassNameW(hwnd, class.as_mut_ptr(), 32);
    if String::from_utf16_lossy(&class[..n as usize]) == "Static"
        && matches!(msg, WM_PAINT | WM_PRINT | WM_PRINTCLIENT | WM_ERASEBKGND)
    {
        if msg == WM_ERASEBKGND {
            return 1;
        }
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let dc = if msg == WM_PAINT {
            BeginPaint(hwnd, &mut ps)
        } else {
            w as HDC
        };
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        let a = &*APP;
        let mut label = vec![0u16; GetWindowTextLengthW(hwnd) as usize + 1];
        let count = GetWindowTextW(hwnd, label.as_mut_ptr(), label.len() as i32);
        let content = String::from_utf16_lossy(&label[..count as usize]);
        let key = (
            a.panel_dark,
            a.panel_scale.to_bits(),
            a.control_background(hwnd),
            content.as_str(),
        );
        a.cached_paint(hwnd as usize, key, dc, r, |buffer| {
            let p = Palette::new(a.panel_dark);
            fill(buffer, &r, a.control_background(hwnd));
            let role = a
                .panel_ui
                .roles
                .get(&(hwnd as usize))
                .copied()
                .unwrap_or_default();
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            text(
                buffer,
                &content,
                r,
                a.panel_font(role),
                if role == TextRole::Caption {
                    p.caption
                } else {
                    p.text
                },
                if style & 3 == 2 { DT_RIGHT } else { DT_LEFT },
            );
        });
        if msg == WM_PAINT {
            EndPaint(hwnd, &ps);
        }
        return 0;
    }
    let now = (*APP).start.elapsed().as_secs_f64();
    if msg == WM_KEYDOWN {
        (*APP).panel_ui.keyboard = true;
    }
    if msg == WM_LBUTTONDOWN {
        (*APP).panel_ui.keyboard = false;
    }
    let result = DefSubclassProc(hwnd, msg, w, l);
    if matches!(msg, WM_SETFOCUS | WM_KILLFOCUS) {
        InvalidateRect((*APP).panel, null(), 0);
    }
    let animate = (*APP).panel_ui.motion_enabled
        && !(*APP).panel_ui.keyboard
        && IsWindowVisible((*APP).panel) != 0;
    if let Some(motion) = (*APP).panel_ui.buttons.get_mut(&(hwnd as usize)) {
        match msg {
            BM_SETCHECK => motion.checked.target(
                if w == BST_CHECKED as usize { 1. } else { 0. },
                now,
                if animate { 0.150 } else { 0. },
            ),
            WM_MOUSEMOVE => {
                // Ignore hover synthesized by touch; keep the existing native
                // button keyboard handling and accessibility semantics.
                if GetMessageExtraInfo() as usize & 0xffffff00 != 0xff515700 {
                    let mut tracking = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    TrackMouseEvent(&mut tracking);
                    motion
                        .hover
                        .target(1., now, if animate { 0.150 } else { 0. });
                }
            }
            WM_MOUSELEAVE => motion
                .hover
                .target(0., now, if animate { 0.150 } else { 0. }),
            WM_LBUTTONDOWN => motion
                .press
                .target(1., now, if animate { 0.100 } else { 0. }),
            WM_LBUTTONUP | WM_CAPTURECHANGED | WM_KILLFOCUS => {
                motion
                    .press
                    .target(0., now, if animate { 0.120 } else { 0. })
            }
            _ => {}
        }
        if motion.active(now) {
            SetTimer((*APP).panel, ANIMATION_TIMER, 16, None);
        }
        if matches!(
            msg,
            BM_SETCHECK | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MOUSEMOVE | WM_MOUSELEAVE | WM_ENABLE
        ) {
            InvalidateRect(hwnd, null(), 0);
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fonts_follow_windows_ui_settings_and_scale_once() {
        unsafe {
            let system = system_ui_font();
            assert!(system.lfFaceName[0] != 0);
            for scale in [1., 1.25, 1.5, 2.] {
                let handle = font(13. * scale, FW_NORMAL as i32);
                assert!(!handle.is_null());
                let mut actual: LOGFONTW = std::mem::zeroed();
                assert_eq!(
                    GetObjectW(
                        handle,
                        std::mem::size_of::<LOGFONTW>() as i32,
                        &mut actual as *mut _ as *mut _
                    ),
                    std::mem::size_of::<LOGFONTW>() as i32
                );
                DeleteObject(handle);
                assert_eq!(actual.lfFaceName, system.lfFaceName);
                assert_eq!(actual.lfCharSet, system.lfCharSet);
                assert_eq!(
                    actual.lfHeight,
                    (system.lfHeight as f32 * scale).round() as i32
                );
                assert_eq!(actual.lfWeight, system.lfWeight);
            }
        }
    }
    #[test]
    fn supersampling_blends_diagonal_edges() {
        unsafe {
            let target = Surface::new(32, 32);
            assert!(!target.bits.is_null());
            let rect = RECT {
                left: 0,
                top: 0,
                right: 32,
                bottom: 32,
            };
            smooth_paint(target.dc, rect, |dc| {
                fill(dc, &rect, rgb(0xffffff));
                line(dc, 3, 5, 28, 23, rgb(0), 1);
            });
            GdiFlush();
            let bytes = std::slice::from_raw_parts(target.bits, 32 * 32 * 4);
            assert!(
                bytes
                    .chunks_exact(4)
                    .filter(|p| p[0] > 0 && p[0] < 255 && p[0] == p[1] && p[1] == p[2])
                    .count()
                    > 10
            );
        }
    }
    #[test]
    fn interruption_keeps_the_current_value_and_settles() {
        let mut motion = Motion::default();
        motion.target(1., 0., 0.15);
        let before = motion.value(0.06);
        motion.target(0., 0.06, 0.15);
        assert!((motion.value(0.06) - before).abs() < 0.00001);
        assert_eq!(motion.value(1.), 0.);
        assert!(!motion.active(1.));
    }
    #[test]
    fn reduced_motion_reaches_the_target_immediately() {
        let mut motion = Motion::default();
        motion.target(1., 2., 0.);
        assert_eq!(motion.value(2.), 1.);
        assert!(!motion.active(2.));
    }
    #[test]
    fn easing_is_monotonic_and_has_exact_endpoints() {
        assert_eq!(ease_out(0.), 0.);
        assert_eq!(ease_out(1.), 1.);
        let mut previous = 0.;
        for step in 0..=100 {
            let value = ease_out(step as f32 / 100.);
            assert!(value >= previous && value <= 1.);
            previous = value;
        }
    }
}
