# 全局角色仓 · 暑假弧角色定义说明

从 `nana_ai/diary/stories` 全量节次扫描后写入 AppData 全局角色仓。GUI「角色定义」可直接编辑。

## TODO（本任务）

| 状态 | 项 | 路径 / 说明 |
|------|----|-------------|
| 完成 | 扫描 stories 角色出场 | `D:\KKFiles\KKProjects\Kinit\nana_ai\diary\stories\`（含 `节次总表.md`、第01–61节） |
| 完成 | 对照人设补全要点 | `.cursor/rules/persona/乐乐/人设.md`、`.cursor/rules/persona/玥玥/人设.md`、`.cursor/rules/persona/娜娜/` |
| 完成 | 写入 unique 角色 JSON | `%APPDATA%\kk_novel_ai\character_roster\lore\characters\*.json` |
| 完成 | 写入世界背景 | `%APPDATA%\kk_novel_ai\character_roster\lore\world\暑假弧_故事世界.json` |
| 可选 | 姨妈/爸妈等配角 | 仅电话/借口出场，未单独建卡；需要再补 |

## 角色清单（unique）

| 角色 | 仓内文件 | 史料锚点 |
|------|----------|----------|
| 娜娜 | `character_roster/lore/characters/娜娜.json` | 全弧；清理侍奉、身侧 |
| 乐乐 | `…/乐乐.json` | 主线高频；真空厌接受 |
| kk | `…/kk.json` | 男主控场 |
| 玥玥 | `…/玥玥.json` | 约第14–16节 |
| 苏晴 | `…/苏晴.json` | 第24–27节 |
| 枝枝 | `…/枝枝.json` | 第28–41、55节等 |
| 晚晚 | `…/晚晚.json` | 第35–37节等 |
| 露露 | `…/露露.json` | 第42–57节 |
| 净净 | `…/净净.json` | 第48、50节 |
| 满满 | `…/满满.json` | 第49节 |
| 慕慕 | `…/慕慕.json` | 第56–58节 |
| 小衡 | `…/小衡.json` | 第56–58节；不碰娜娜 |

## 硬规则摘要（已写进 content）

- 全员成年；女体角色无阴茎。
- 主线男性通常为 kk；外男宾（如小衡）不碰娜娜。
- 乐乐：超短裙真空、讨厌但接受、对白含蓄。
- 娜娜：§7A 清理；自称成套。

## 相关代码 / 产品路径

- 角色仓根：`%APPDATA%\kk_novel_ai\character_roster\`
- 前端页：`src/views/CharacterRosterView.vue`
- Lore 合并：`src-tauri/src/writing/mod.rs`（`linked_kb_roots` 含 `@characters`）
- 节次索引：`nana_ai/diary/stories/节次总表.md`
