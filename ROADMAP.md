# Roadmap

## Released

### v0.2.0
- 全局快捷键触发录音（⌘⇧Space）
- Groq Whisper Large V3 Turbo 云端转录
- AI 润色层（标点修正、错字纠正）
- 文字自动注入 / 剪贴板回退
- 系统托盘 + 设置面板
- macOS & Windows 支持
- 开机自启

### v0.3.0
- 截图上下文润色：录音触发时截取屏幕，以多模态 Vision 模型（llama-4-scout-17b-16e-instruct）提升润色准确度

### v0.3.1
- HUD 与设置面板独立位置记忆，支持独立拖拽
- 修复录音结束后设置面板被意外关闭的问题
- 截图捕获异步化，不阻塞录音启动

---

## Pre-Launch Checklist（推广前必做）

### 分发与信任基础设施

- [ ] **Apple Developer 账号 + 代码签名 + 公证**：消除 Gatekeeper "未知开发者"拦截，$99/年，一次性流程
- [ ] **隐私声明**：明确告知用户音频上传至 Groq（当前架构）、截图仅本地处理、无其他数据收集；本地推理上线后更新为"完全本地"
- [ ] **落地页（单页）**：核心演示 GIF + 一句话介绍 + 下载按钮，参考 lookaway.com 风格
- [ ] **Homebrew Cask 提交**：面向开发者群体，`brew install --cask` 比手动下载 .dmg 信任感更高
- [ ] **首发渠道**：Hacker News "Show HN"、少数派、V2EX

### 商业化路径

- [ ] **当前过渡期：BYOK 模式**（用户自带 Groq API Key）：免费分发，零运营成本，降低用户决策门槛
- [ ] **托管 API 订阅（Pro $3/month）**：无需 API Key，包含 20 hrs/month 转录；详见 [docs/pricing-strategy.md](docs/pricing-strategy.md)
- [ ] **本地推理上线后：买断制**（目标定价 $9–15，参考 lookaway $9.99）：零边际成本，一次收费
- [ ] **（可选）订阅增强版**：词汇表云端同步、多设备支持等持续价值功能，届时再评估

---

## Planned

### v0.4.0 — 用户自定义词汇表（User Vocabulary）

**动机：** Whisper 对低频专业词汇（医学、运动科学、行业术语等）识别率低，即使有截图上下文也难以纠正；用户需要一种轻量的方式告诉系统"我会说这些词"。

**方案：**
- 设置面板新增"词汇表"入口，用户手动维护专属词汇（每行一词，或 CSV 格式）
- 录音触发时，将词汇表作为 hint 注入润色 prompt：
  ```
  用户自定义词汇（优先保留/纠正为这些写法）：髂胫束、跖骨疲劳性骨折、IT band …
  ```
- ASR 转录完成后，LLM 可对照词汇表进行音近纠错（如"卡经术" → "髂胫束"）
- 词汇表存储于本地 JSON/SQLite，不上传

**prompt 开销估算：** 100-200 词约 500-1000 token，可接受。

**后续扩展（v0.5.0+）：** 可结合使用记录自动推荐词汇（用户多次手动修正某词后自动提示加入词汇表）。

---

### 本地化推理（完全离线 / 零数据上传）

**动机：** 隐私敏感用户（企业、开发者）的核心诉求；彻底开源、无 API 依赖、可离线使用。

**架构目标：**
```
语音 → whisper.cpp (本地 + Metal/DirectML 加速)
         ↓ 转录文字
       本地多模态 LLM (llama.cpp / mlx)
         + 屏幕截图 context
         ↓ 润色文字
       注入目标输入框
```

**候选模型：**

| 用途 | 模型 | 量化大小 | 备注 |
|------|------|---------|------|
| 转录 | Whisper Large V3 Turbo | ~600 MB (q5) | 已验证效果 |
| 润色/Vision | Qwen2.5-VL 3B | ~2 GB (q4) | 视觉理解强，轻量 |
| 润色/Vision | Qwen2.5-VL 7B | ~5 GB (q4) | 精度更高，需 16GB+ |
| 润色/Vision | Gemma 3 4B | ~3 GB (q4) | Google 最新，支持 vision |

**推进步骤：**
1. 集成 `whisper.cpp` 替换 Groq Whisper（优先，成熟度最高）
2. 集成 `llama.cpp` / `mlx-lm` 跑本地润色模型
3. 模型按需下载（首次启动自动拉取，避免 bundle 膨胀）
4. 云端 / 本地模式切换开关（保留 Groq 作为备选）
5. Windows DirectML / CUDA 支持

**已知挑战：**
- App bundle 分发：模型文件 3–6 GB，需要首次下载流程
- 内存压力：8 GB Mac 同时 load Whisper + VL 模型需要 load/unload 策略
- Windows 本地推理方案比 macOS Metal 复杂
