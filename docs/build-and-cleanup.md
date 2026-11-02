# 构建与产物清理

编译和打包沿用根 workspace、锁文件和固定工具链。各平台仅构建主程序，共享根 workspace 与锁文件。

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

`scripts/build.py` 每次编译成功后立即清理仓库 `target/` 下所有 `debug/release` 构建目录（含目标平台子目录）：各目录只保留最新 `herdr-work` / `herdr-work.exe` 和 Cargo 锁文件，删除 `deps`、`build`、`.fingerprint`、`incremental`、测试程序及旧程序产物。清理后下次编译需要重新构建依赖。编译失败时不执行这一步，保留产物以便排查。直接运行 `cargo build/test` 不触发脚本清理。

编译前仍清理超过 7 天未写入的增量缓存和脚本 Python 字节码；时间判断包含增量目录的所有子项。各类缓存清理均非阻塞获取对应 profile 的 `.cargo-lock`、`.cargo-build-lock` 和 `.cargo-artifact-lock`，Windows 直接锁定空锁文件的字节范围，不向被 Cargo 锁定的文件写入标记。目录正在使用时跳过，并在日志 `busy` 字段列出，不等待或停止进程。

清理不跟随符号链接，只处理默认仓库 `target/`，不清理其他 `CARGO_TARGET_DIR`、Cargo 全局依赖下载、用户配置、安装回滚备份或独立插件目录。`.scratch/` 中的规格、截图和验收证据保留。

打包先在输出目录的独立临时目录中生成完整归档和 SHA256，验证文件集、许可和内部校验和，再替换正式产物。写入校验和失败时恢复上一份正式归档；候选构建或验证失败不清理旧归档。正常退出自动删除临时目录；输出目录中超过 7 天的 `.herdr-work-package-*` 中断残留在取得打包锁后清理。

新归档成功后，同平台默认保留 1 份，其他平台不受影响。独立清理按语义版本号保留最新版本，正式版优先于同版本的预发行版，不受复制或修改文件时间影响；打包时始终保留本次通过验证的候选，包括显式构建旧版本的情况。只有符合产品归档文件名、具有同名 SHA256 文件且校验一致的归档参与清理；未知文件和无有效校验和的文件保留。可增加保留数量：

```sh
python3 scripts/package.py create --target aarch64-apple-darwin --keep-archives 2
```

保留多份时，将需要验证的单份归档放入独立目录后运行 smoke；该命令要求目录中恰有一个当前平台归档。

## 手动预览与清理

`sh herdr-work-plugins.sh clean` 会立即清理所有空闲的增量缓存和脚本字节码，各平台只保留最新版本的有效归档；此手动命令保留依赖库、当前二进制和安装备份。运行中的 Cargo profile 跳过，并在输出的 `busy` 字段列出。脚本编译成功后自动执行更彻底的中间产物清理。

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

脚本回归覆盖编译成功后清理、失败保留产物、保留最新主程序、占用跳过、符号链接边界、手动清理保留近期缓存、嵌套写入时间、其他平台与无主文件保留、候选失败及归档替换失败恢复。macOS 的真实构建与隔离安装结果单独记录；三平台构建前执行构建清理回归；Windows 原生用例覆盖已被锁定的空 Cargo 锁文件，Unix 用例覆盖文件锁与符号链接边界。
