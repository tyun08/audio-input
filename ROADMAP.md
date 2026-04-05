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

## Planned

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
