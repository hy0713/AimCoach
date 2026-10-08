# AimCoach

Windows 本地练枪成长教练，第一版读取 KovaaK 的 Stats CSV。工程目录名为 `aimhelper`，应用名称为 **AimCoach**。

采用 Tauri 2、Vue 3、TypeScript、Vite、Element Plus、Pinia、ECharts；Rust 负责只读采集、SQLite、统计、Benchmark、计划与规则教练。普通功能完全离线，无账户和云数据库。

## 启动

源码仓库不包含安装包、开发工具链和个人训练数据。运行 `npm run package` 可生成 Windows 安装包，输出在 `src-tauri/target/release/bundle/nsis/`。安装后的程序不需要 Node 或 Rust，但需要 Windows 的 WebView2 Runtime。

开发命令，在 PowerShell 中运行：

```powershell
cd AimCoach
npm ci
npm run desktop
```

本次在项目 `.tools/` 中安装了 Rust GNU 工具链；`scripts/desktop.mjs` 自动使用它及本机的 `E:\mingw64\bin`。**本机已验证这个组合，项目不把此路径当作其他机器的通用前提。** 其他 Windows 开发机建议按 [Tauri 官方前置条件](https://v2.tauri.app/start/prerequisites/) 安装 Rust MSVC、Visual Studio C++ Build Tools 和 WebView2，再执行上述命令。脚本在没有项目内工具链时使用系统 PATH。

```powershell
npm test                      # 独立 Rust 核心测试，不链接 WebView2
npm run test:ui               # 浏览器预览、空库页面、AI 授权交互
npm run check:desktop         # Rust 桌面命令编译检查
npm run build                 # TypeScript 类型检查 + Vue 生产资源
npm run package               # 优化版 Windows exe + NSIS 安装包
node scripts/desktop.mjs build --debug --no-bundle # 生成独立调试程序
npm run test:native           # 实际 Tauri/WebView2、监听、数据库和图表验收
```

UI 测试默认使用本机 Chrome。其他路径设置 `AIMCOACH_BROWSER`。原生验收脚本默认测试 `src-tauri/target/debug/aimcoach.exe`，支持 `--release` 测试优化版；它将测试库放在 `test-output/`，**不会给正式用户数据库填入合成成绩**。仅在测试程序进程中启用 WebView2 调试端口 15422，结束后关闭该进程。开发端口为 15420，本机 1420 端口不可监听。

`npm run dev` 是浏览器界面预览，不含本地数据库后端；页面会明确提示这一点。

## 首次使用

1. 打开设置。应用尝试 Steam 注册表、默认 Steam 路径、Steam Library 配置和常见盘符目录自动定位。
2. 自动定位失败时，点击“选择目录”，找到 `steamapps\common\FPSAimTrainer\FPSAimTrainer\stats`。
3. 点击“扫描历史记录”。“重新解析全部历史”会重新校验并更新同一次训练，不清空历史。
4. 开启“自动监听”并保存设置。新增/变更 CSV 稳定后导入；可查看新增、更新、跳过、失败及暂缓数和详细错误。
5. 在“场景与成长”查看历史成绩、PB 曲线、最近10次中位数、7/30天变化、Accuracy 与波动。
6. 本地教练报告可随时生成；成功导入新记录或更新记录后，也会保存本地总结。
7. 若需跨场景能力比较和具体训练计划，先导入经过核实的 Benchmark JSON。

应用数据默认在 `%APPDATA%\dev.aimcoach.desktop\aimcoach.sqlite`，实际路径在设置页显示。`AIMCOACH_DATA_DIR` 可指定独立数据目录（用于测试或便携使用）。SQLite 备份通过设置页导出，备份目标必须为新的 `.sqlite` 文件并位于 Stats 目录之外。备份包含原始字段映射，请自行保管。

## CSV 兼容性与数据含义

已检查本机 **KovaaK 3.9.5 / 3.9.10** 实际 CSV；289 个文件导入成功，74 个场景，重复扫描新增0条。验证结果见 [docs/VERIFICATION.md](docs/VERIFICATION.md)。这不代表所有历史版本或导出格式都已验证。

支持：

- 分段 CSV：击杀明细、武器总量、键值摘要、设置字段；可变列数、引号、特殊字符。
- 明确的单行摘要表（Scenario Name、Score、时间等）；不接受多训练行混在一个文件的汇总导出。
- UTF-8、UTF-8 BOM、带 BOM 的 UTF-16 LE/BE。无法可靠解码的旧编码会报错，不猜测编码。
- 字段缺失保存 NULL；保留所有原始行、已识别字段、解析来源与解析器版本。
- Score 必须来自训练摘要，不用击杀表推算。Shots/Hits 优先采用武器总量；Accuracy 缺失时可从完整 Shots/Hits 计算，并标注来源。
- `Challenge Start` 为完整日期时直接使用。3.9 文件中的时分秒结合 Challenge 文件名日期解析；跨午夜仅在时差超过12小时情况下回退一天。文件名作为时间后备时有独立来源标记，**不把文件修改时间当训练时间**。
- 本机文件中 `Fight Time` 不是完整训练时长：跟踪场景可为0，切换场景只累计战斗时间。因此仅在存在明确 `Duration` / `Challenge Duration` 字段时填充完整时长；本机289条的完整时长均为 NULL。战斗时间保留在原始字段中。

UI 显示训练时间及来源。若缺少明确完成时间，使用可验证的开始时间，不把它伪称为完成时间。完整训练时长的缺失也不会用“每局60秒”等假设补齐。

文件修改不到2秒时暂缓；新/变更文件必须经过两次内容与元数据检查。内容 SHA-256 用于扫描跳过；训练身份按来源、场景和秒级规范时间构造，避免复制文件产生重复记录。相同训练身份的更正数据会更新记录。极端情况下同一场景同一秒开始的两局会被视为同一训练；后续可使用更精确的源身份扩展。

单文件最多32MB，错误文件隔离，失败原因最多显示100条。递归扫描跳过文件系统符号链接。批量导入在 Rust 阻塞任务线程中执行，UI 可继续导航。自动监听使用 notify、去抖与暂缓重检。原始文件仅以读方式打开，应用没有游戏进程注入、输入模拟、覆盖层或屏幕采集功能。

格式依据：

- [KovaaK 官方 Performance Files 文档](https://wiki.kovaaks.com/en/home/KovaaK%27s/PerformanceFiles)：确认 Stats 目录和挑战文件命名；本版不解析 `.perf`。
- [KovaaK 官方支持文档](https://wiki.kovaaks.com/en/home/KovaaK%27s/Support)：统计导出配置和文件位置。
- [Bredgren/KovaakStats 原始解析实现](https://github.com/Bredgren/KovaakStats/blob/master/kovaakStats.py)：参考分段结构和字段名称；本工程未复制其算法或硬编码成绩调整。
- 本机3.9实际文件格式检查及合成回归样本。

官方页面对文件名时间的描述与本机文件中的 `Challenge Start` 存在差异，所以优先读取显式开始时间并展示来源，未将所有文件名一律当完成时间。

## Benchmark 配置

`config/viscose-s2-easier.json` 已提供 Viscose S2 Easier 的 39 个场景、4 大类、14 子类、9 等级和 351 个门槛。2026-10-08 从 [EVXL](https://evxl.app/) 实际加载的 KovaaK 公开接口获取，并逐项与网页成绩表核对。使用「设置 → Benchmark JSON → 从文件载入 → 导入」安装配置；不自动改动已有配置。公开配置不含玩家标识或个人成绩。

`config/viscose-s2-easier.template.json` 保留为空模板。`tests/fixtures/benchmark.synthetic.json` 是明确标记的测试配置，不是 Viscose 数据，不会自动安装到应用中。个人成绩快照、数据库和本机导入脚本不在源码仓库中。详见 `docs/EVXL.md`。

配置结构如下（下面的场景和分数只用于解释结构，正式使用请填写真实数据）：

```json
{
  "name": "你的已核实 Benchmark",
  "version": "S2",
  "difficulty": "Easier",
  "source": "https://你的可靠公开来源",
  "ranks": ["等级 A", "等级 B"],
  "scenarios": [{
    "name": "真实场景的完整名称",
    "aliases": ["Stats 中的另一个准确名称"],
    "category": "Control Tracking",
    "subcategory": "Smoothness",
    "tags": ["Smoothness", "Stability"],
    "thresholds": {"等级 A": 500, "等级 B": 1000}
  }]
}
```

等级、场景和别名不得重复；每场景必须有全部等级门槛，有限正数且严格递增；不自动填补缺失门槛。名称匹配规范空白和大小写，其他差异需要显式 aliases。Settings 支持文件载入、JSON 编辑、校验和导入；Benchmarks 显示匹配到的名称和未匹配场景。

分类来自配置，支持 Control Tracking、Reactive Tracking、Flick Tech、Click Timing，也允许其他大类和子类。版本和难度都是配置字段，可导入 Medium/Hard/Expert；当前管理一个活动 Benchmark，导入新配置会替换活动配置，训练历史保留。

归一化单位是“等级序号 + 等级内进度”。例如第三等级的65%进度为3.65；低于首级使用0到首级之间的比例；达到最高等级后封顶。PB 等级用于等级数量、达级记录和 Complete；日常能力使用各场景最近最多10次中位数归一化，场景等权平均，同时显示最高PB、均值与最低表现、覆盖率及样本可信度。未训练场景计入 Complete 分母，不用已有最好场景代表整个子类。

## 分析规则

计算全部在 Rust 模块中，Vue 负责渲染与筛选。日期范围筛选由后端重新计算。

- 前后对比用最多各10次、不重叠的成绩窗口，总样本不足10次标记“数据不足”。
- 稳定改善：近期中位数提升超过2%，近期两段中位数都比过去高2%以上，波动未明显扩大。只是近期表现证据，不承诺长期技能因果关系。
- PB 上升但中位数下降超过2%：标记“仅 PB 提升”，不判稳定进步。
- 7/30天分别比较最近窗口与紧邻的上一同长度窗口；各至少5次才输出变化。
- CV 是同场景最近最多10次成绩的相对标准差。异常偏离使用 MAD 和15%最低偏离幅度，至少10次。
- 平台期：最近30天至少30次、跨度至少21天、前后中位数变化小于1%、后半段CV低于10%。显示“可能”，不当作确定诊断。
- 会话：连续记录间隔不超过设置阈值，默认20分钟，这是产品规则。
- 衰减：一个会话内同场景至少9次，分三段；后一段中位数连续下降超过3%才提示可能与疲劳有关。不同场景的原始 Score 不会混合比较。

计划从活动 Benchmark 的真实场景库选择，根据主攻项、弱项、训练覆盖、近期变化、超过7天未练和保留强项偏好分配分钟，并列出理由；分钟总和等于计划时长。无 Benchmark 时不会编造推荐场景。满足样本量与跨度规则的平台期会增加所属大类的训练权重，并优先轮换该类别的其他已配置场景。

## 可选 AI

本地规则报告无需网络。开启 AI 的操作顺序：填写 Base URL 和 Model → 保存 API Key → 明确确认聚合数据离开本机 → 保存配置和授权 → 教练页主动点击“请求 AI 补充解释”。API Key 由 Rust keyring 写入 Windows 凭据管理器，不回填到 UI，不写数据库、备份或日志。

调用 Chat Completions 接口；外部服务必须 HTTPS，本机 localhost/127.0.0.1 可使用 HTTP。禁止自动跟随重定向，超时35秒，不自动重试付费调用。只发送场景名称、样本数量、预计算统计、训练目标、计划及会话衰减摘要；不发送文件路径、逐次原始记录或原始CSV。基础数学来自本地算法，模型负责补充解释。

未授权时后端拒绝调用。API、凭据或响应失败时保存完整本地报告和回退提示。AI 内容与本地确定性结论明确分开；不在预计算统计中的百分比会被拒绝（允许一位小数舍入）。提示词要求证据约束，但**LLM 输出仍可能不准确，尚未实现逐句事实认证**。本次未使用真实外部API Key发起付费调用，成功响应的各供应商兼容性待验证；网络不可用回退已有实际测试。

## 工程结构

```text
src/
  pages/                 六个中文页面
  components/            ECharts 组件与能力图
  store.ts               Pinia、Tauri IPC 和同步状态
  types.ts               前端数据类型
src-tauri/
  migrations/            按 PRAGMA user_version 执行的增量迁移入口
  src/parser.rs          RunParser 接口与 KovaaK CSV
  src/importer.rs        定位、只读扫描、稳定读取、指纹去重
  src/storage.rs         SQLite、实体存储、会话归组、备份
  src/analysis.rs        中位数、趋势、波动、平台期、衰减
  src/benchmark.rs       配置校验、匹配、等级、分项与 Complete
  src/coach.rs           规则报告、计划、可选 AI
  src/desktop.rs         异步 Commands、notify 生命周期、凭据
  tests/core.rs          核心验收测试
  examples/validate_stats.rs  只读真实文件验证工具
tests/fixtures/          明确标记的合成测试数据
tests/ui/                浏览器交互验收
scripts/native-smoke.mjs 实际 WebView2/Rust/SQLite 验收
```

未来 Aim Lab 可实现统一 `RunParser` 与来源身份，不需要将算法写入 UI。本次未开发 Aim Lab、画面/录像分析、鼠标操作、账号、排行榜、云同步或 `.perf`。原始字段保留为将来增加更细指标提供入口，不依据当前汇总 CSV 判断 overshoot、Y轴误差或重新捕获延迟。

后续优先：补充其他KovaaK版本样本；更精确的源训练身份、长历史分页和按日对比；获得用户授权后验证指定AI提供方；安装包签名与多机Windows10/11安装测试。
