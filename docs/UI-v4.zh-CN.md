# v2.5.0 · UI v4：Windows 界面优化版

基于 [DLmember/Minecraft-Desktop-Pet](https://github.com/DLmember/Minecraft-Desktop-Pet) 的独立 Fork 分支，保留原项目历史与许可。

## 主要变化

- 原生 Win32 控制台采用侧栏和卡片布局，支持深浅主题与按钮、开关反馈。
- 默认及最小客户区为 760×700 逻辑像素，宽高可独立调整。
- 三倍分辨率抗锯齿绘制，并使用 Windows 当前系统界面字体。
- 托盘菜单提供“快捷操作 / 桌面设置”和分组子菜单，修复切页背景缺失。
- 修复 Q 版身体大小滑块拖动时的锯齿残影。

## 展示图

![浅色控制台](https://github.com/Kevae12138/Minecraft-Desktop-Pet/raw/refs/heads/windows-ui-v4/docs/images/console-light.png)

![深色控制台](https://github.com/Kevae12138/Minecraft-Desktop-Pet/raw/refs/heads/windows-ui-v4/docs/images/console-dark.png)

![托盘桌面设置](https://github.com/Kevae12138/Minecraft-Desktop-Pet/raw/refs/heads/windows-ui-v4/docs/images/tray-desktop-dark.png)

## 使用

Windows 10/11 x64。推荐下载 ZIP，解压后运行 `Minecraft桌宠.exe`，其中包含使用说明与许可证。也可单独下载 EXE。

运行前先从托盘退出旧版。双击托盘图标打开控制台，右键打开快捷菜单。程序内检查更新仍指向上游；本 Fork 的 UI 更新请查看本仓库 Releases。

## 验证

Windows 二进制测试 81 通过、1 项原有忽略、0 失败。五页深浅布局、独立窗口尺寸、100 次导航、80 次托盘切页、真实屏幕滑块拖动与键盘调节、系统字体通知均通过。测试使用独立临时用户目录。

实际屏幕回归在本机 125% DPI 下完成；字体的 100%、125%、150%、200% 缩放由单元测试覆盖。控制台图片由程序绘制导出，托盘与滑块采用实际屏幕截图。

SHA256 校验值见随附的 `SHA256SUMS.txt`。基础版本号保持 2.5.0，UI 修订号为 v4。本次不提供新的 macOS 二进制。
