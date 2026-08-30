/**
 * 移动端验收清单（浏览器窄屏 + 真机）
 * 代码路径: docs/mobile-qa-checklist.md
 */

## 视口

- [ ] 360×800 / 412×915：底栏可见，侧栏为抽屉
- [ ] 隐藏桌面标题栏三键（最小化/最大化/关闭）
- [ ] safe-area：刘海/手势条不挡底栏与 FAB

## 作品 / 设置

- [ ] 新建作品写入应用 novels 目录提示可用
- [ ] 导入 ZIP 备份 / 导出 ZIP 备份可下载
- [ ] 手机不出现「打开其它目录」
- [ ] 设置页 Base URL 提示局域网/HTTPS；保存 localhost 应失败（真机）

## 写作

- [ ] 目录抽屉开关正常
- [ ] AI 强制浮条，无 320px 侧栏挤占
- [ ] 软键盘弹起时可编辑；跳转 FAB 不被挡住
- [ ] 流式生成时滚动不被强制抢走

## 构建

- [ ] `python build.py --platform windows --no-bump` 产出 EXE
- [ ] 安装 Android SDK 后 `npm run tauri -- android init`
- [ ] 配置签名后 `python build.py --platform android --no-bump` 产出 APK
