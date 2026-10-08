# EVXL Benchmark 配置来源

核对日期：2026-10-08。配置来自 [EVXL](https://evxl.app/) 页面实际加载的 KovaaK 公开 Benchmark 接口，并和网页成绩表逐项校验。

## 已核对的数据

- Viscose Benchmarks S2，Easier，Benchmark ID 为2335。
- 39场景、4大类、14子类、9等级、351个门槛。
- 等级：Lemming、Hare、Ermine、Puffin、Penguin、Fox、Mammoth、Orca、Seal。
- 场景名称、等级顺序和每一项门槛均与页面一致。
- 接口中的场景 score 为网页 Score 的100倍；rank_maxes 已是网页分数单位。
- 接口以末尾空格区分重复子类名（如 Speed / Speed 后附空格）。配置使用大类与子类共同标识，避免混合不同技能类别。

## 使用

在 AimCoach 设置页，点击「从文件载入」，选择 config/viscose-s2-easier.json，再点击「校验并导入配置」。导入后离线可用。

已经通过实际 Windows 应用的 Rust Commands 验证：配置正确导入，本机真实 CSV 中39个 Benchmark 场景全部按名称匹配。无需猜测别名。

## 数据含义与范围

网页当前场景成绩是快照，不能代表每次完整训练历史。若没有单条成绩时间、训练次数、时长或 Accuracy，应保持未知，不能用抓取时间充当训练时间。

AimCoach 的 TrainingRun 历史仍来自本地 Stats CSV；稳定进步、中位数、波动、平台期和疲劳提示都由本地记录计算。EVXL 的总等级与子类汇总采用其自己的规则，不能等同于 AimCoach 的近期稳定表现。

此仓库只发布公共场景定义和门槛，不包含个人成绩、Steam ID、原始接口响应或训练数据库。捕获工具和个人导入脚本留在原开发机器，尚未提供通用 EVXL URL 导入界面。

配置是核对当日的副本。发布者更新 Benchmark 时，需要重新核对并导入新配置。
