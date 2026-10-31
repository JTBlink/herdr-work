# 构建与产物清理

编译和打包沿用根 workspace、锁文件和固定工具链。两个 Unix 产品程序由一次 Cargo 调用构建，共享依赖编译；Windows 只构建主程序，Projects 执行后端仍仅支持 Unix。

## 一键编译打包

macOS Apple Silicon / Linux x86_64 在仓库根执行：

```sh
sh herdr-work-plugins.sh --build-only  # 自动识别平台，编译并打包到 dist/
sh herdr-work-plugins.sh install       # Release 编译打包后安装到 bin/，并链接到 Herdr
sh herdr-work-plugins.sh --release     # 通过 Herdr 安装 VERSION 对应的发行版
sh herdr-work-plugins.sh clean         # 清理空闲增量缓存和历史归档，不需要 Cargo 或 Herdr
```

不传参数等同于 `install`；源码模式使用 Cargo `--release --locked`，需要 Rust 1.99.0、Python 3.12+、Git 和平台编译工具。`--build-only` 不需要 Herdr，也不修改已安装程序、快捷键或插件注册；默认安装需要 Herdr 0.9.3+。可从任意目录通过脚本绝对路径调用。归档及 `.sha256` 保留到仓库 `dist/`，同平台只保留最新一份；失败不清理上一份可用归档。源码安装链接当前仓库，请保留仓库目录。

## 分步构建与验证

```sh
python3 scripts/build.py --target aarch64-apple-darwin
python3 scripts/package.py create --target aarch64-apple-darwin
python3 scripts/smoke.py --archives dist
```

`CARGO_TARGET_DIR` 同时作用于编译和打包，未设置时使用仓库根 `target/`。相对路径以仓库根解析；构建显式传递 `--target-dir`，避免 Cargo 配置和打包脚本读取不同目录。脚本返回的元数据保持在标准输出，清理日志写入标准错误。

## 自动清理

编译前清理仓库 `target/` 内超过 7 天未写入的 `debug/release/incremental` 缓存，包含目标平台子目录；脚本目录中超过 7 天的 Python 字节码文件也会清理。时间判断包含增量目录的所有子项，近期仍在使用的缓存保留。

清理增量缓存前非阻塞获取 Cargo 对应 profile 的 `.cargo-lock`；目录正在编译时跳过，不等待或停止进程。清理不跟随符号链接，不清理外部 `CARGO_TARGET_DIR`、Cargo 全局依赖下载、编译后的依赖库、用户配置、任务数据、安装回滚备份或独立插件目录。`.scratch/` 中的规格、截图和验收证据保留。

打包先在输出目录的独立临时目录中生成完整归档和 SHA256，验证文件集、许可和内部校验和，再替换正式产物。写入校验和失败时恢复上一份正式归档；候选构建或验证失败不清理旧归档。正常退出自动删除临时目录；输出目录中超过 7 天的 `.herdr-work-package-*` 中断残留在取得打包锁后清理。

新归档成功后，同平台默认保留 1 份，其他平台不受影响。独立清理按语义版本号保留最新版本，正式版优先于同版本的预发行版，不受复制或修改文件时间影响；打包时始终保留本次通过验证的候选，包括显式构建旧版本的情况。只有符合产品归档文件名、具有同名 SHA256 文件且校验一致的归档参与清理；未知文件和无有效校验和的文件保留。可增加保留数量：

```sh
python3 scripts/package.py create --target aarch64-apple-darwin --keep-archives 2
```

保留多份时，将需要验证的单份归档放入独立目录后运行 smoke；该命令要求目录中恰有一个当前平台归档。

## 手动预览与清理

`sh herdr-work-plugins.sh clean` 会立即清理所有空闲的增量缓存和脚本字节码，各平台只保留最新版本的有效归档；依赖库、当前二进制和安装备份保留。运行中的 Cargo profile 跳过，并在输出的 `busy` 字段列出。日常编译仍保留 7 天内的增量缓存以加快重复构建。

```sh
# 预览候选路径、估算文件字节数及因编译占用而跳过的目录
python3 scripts/cleanup.py

# 执行相同策略
python3 scripts/cleanup.py --apply

# 立即清空空闲的增量缓存和 Python 字节码，同平台归档保留两份
python3 scripts/cleanup.py --apply --max-age-days 0 --keep-archives 2
```

独立清理命令只处理仓库根 `dist/` 中的归档；打包时使用 `--output` 的自定义目录由该次打包按同样的保留策略处理。预览不删除候选项，可能创建用于协调编译或打包的锁文件。

## 验证

```sh
python3 -m unittest discover -s scripts/tests -v
```

脚本回归覆盖保留近期缓存、嵌套写入时间、占用跳过、符号链接边界、其他平台与无主文件保留、候选失败及归档替换失败恢复。macOS 的真实构建与隔离安装结果单独记录；Linux/Windows 原生编译和文件锁行为仍由对应平台 CI 验证。
