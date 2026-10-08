# AimCoach 验证记录

日期：2026-10-08。所有结论来自本次命令执行或实际应用验收，不把静态界面当作真实用户数据。

## 核心测试

`npm test`：**33 passed，0 failed**。测试采用合成 CSV 和独立临时 SQLite。

覆盖重复导入和复制去重、更正文件更新、按训练时间排序、坏文件隔离、UTF-8/UTF-16、特殊字符、完整摘要、3.9时分秒与短武器行、跨午夜、缺失值NULL、PB上涨而中位数下降、持续改善、样本不足、等级边界和区间进度、跨场景归一化、子类全场景聚合、未配置Benchmark场景、配置校验与别名、迁移重开保留、空库教练、实际localhost API失败回退、未授权阻止调用、AI聚合证据不含路径、计划时长守恒、会话与同场景衰减、平台期门槛、平台期影响训练计划、AI地址结构校验、AI未支持百分比拒绝。

## 前端

`npm run build`：Vue TypeScript 类型检查与 Vite 构建通过。图表、组件库和框架拆分为独立chunk；仍有组件库大chunk提示，不影响本地构建或使用。

`npm run test:ui`：**3 passed**。实际Chrome中验证浏览器预览身份、六页空数据导航、AI授权必须确认且可取消。空库测试使用明确隔离的IPC响应；它不替代下面的实际桌面验收。

## 实际 Windows 桌面

已运行生成的 `aimcoach.exe`，创建标题为“AimCoach · 练枪成长教练”的Windows窗口。

`scripts/native-smoke.mjs` 通过实际 WebView2 调试接口操作Tauri页面，调用实际Rust Commands，使用项目内独立SQLite和合成Stats目录。结果：

- 空库六页面正常进入。
- 开启真正的notify监听，先后写入两份合成CSV，自动入库2条。
- 重复扫描新增0条、跳过2条。
- 合成Benchmark JSON正确计算Test Silver。
- 本地教练报告成功保存。
- SQLite备份成功导出。
- 场景页面显示实际后端提供的历史记录及ECharts曲线。

这是实际Tauri/Rust/SQLite流程，不是静态假数据演示。测试库、日志和截图在 `test-output/native-*/`；没有把合成配置或记录写入正式用户数据库。

最终优化版额外验证：关闭监听后，Rust监听状态与UI“手动同步”状态一致。

## 真实 KovaaK CSV

只读检查本机Stats文件；未复制原始CSV到测试样本，未修改源文件。

`validate_stats` 在项目内独立验证库执行两次扫描：

| 指标 | 结果 |
| --- | ---: |
| 原始CSV数量 | 289 |
| 首次新增 | 289 |
| 首次失败 / 暂缓 | 0 / 0 |
| 重复扫描新增 | 0 |
| 重复扫描跳过 | 289 |
| 数据库训练记录 | 289 |
| 场景数 | 74 |
| 时间排序 | 正确 |
| Accuracy可靠可提取 | 289 |
| 完整训练时长可靠可提取 | 0 |
| 本地规则报告 | 已生成 |

实际文件来自3.9.5和3.9.10。`Fight Time` 不是完整训练时长，保留原始字段且duration为NULL。时间优先使用Challenge Start；若只有时分秒，则结合文件名日期，并保留来源标记。

`scripts/native-real.mjs` 进一步通过优化版应用的真实Rust Commands导入这些文件：289条、74场景、0失败，重复扫描0新增/289跳过；重新加载应用后历史保留，真实场景曲线与自动保存的本地报告均已验证。结果在 `test-output/native-real-1791427345071/result.json`，截图为同目录 `real-history.png`。

## Windows 构建产物

`npm run package` 成功生成优化版x64程序与NSIS安装包，安装包约6.9MiB。

交付文件：`delivery/AimCoach_0.1.0_x64-setup.exe`、`delivery/AimCoach-portable/`、`delivery/SHA256SUMS.txt`。便携版脚本将数据库保存在该目录的data文件夹；直接运行exe使用默认APPDATA目录。

## 已知验证边界

- 已核对 Viscose S2 Easier 的39场景和351门槛，并验证配置导入、真实场景名称匹配；详见 EVXL 文档。
- 未使用用户付费AI Key；已测试真实网络失败与本地回退，未验证外部供应商成功调用。
- 未证明所有KovaaK版本兼容，未知编码或无法识别的摘要会给具体错误。
- 当前机器通过桌面与数据流程验收；尚未在另一台Windows10/11机器安装验收，也未签名安装包。
- 本机GNU工具链下，直接带桌面特性的 `cargo test` 可能从依赖构建目录加载无效WebView2 loader；标准 `npm test` 刻意独立测试核心，原生验收使用由Tauri CLI正确准备Loader的桌面程序。
- SQLite当前迁移版本为1。重开与历史保留已测试；未来迁移必须追加版本脚本，不能删库重建。
