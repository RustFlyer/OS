目前架构如图，已写代码文件位于 `kernel/src` 和 `lib/config` 和 `lib/logger` 目录下

于主目录下

键入 `make build2docker` 即可构建docker环境，

键入 `make docker` 即可进入docker环境，

键入 `make run` 即可运行

键入 `make run-debug` 即可运行debug模式，此模式下会打印when_debug!宏的输出

键入 `make run-docker` 即可在docker中运行

键入 `make run-docker-debug` 即可在docker中运行debug模式，此模式下会打印when_debug!宏的输出

```
kernel
├── Cargo.toml
├── Makefile           ---- 编译文件
├── build.rs           ---- link.ld地址替换
├── link.ld            ---- 链接脚本
└── src
    ├── console.rs     ---- 打印函数
    ├── entry.S        ---- 入口函数
    ├── lang_item.rs   ---- 崩溃处理
    ├── logging.rs     ---- 日志打印函数
    ├── main.rs        ---- 主函数
    └── sbi.rs         ---- sbi函数

lib
├── config             ---- 配置文件
│   ├── Cargo.toml
│   └── src
│       ├── lib.rs
│       └── mm.rs
├── logger             ---- 日志打印文件
│   ├── Cargo.toml
│   └── src
│       └── lib.rs
└── simdebug           ---- 调试宏
    ├── Cargo.toml
    └── src
        └── lib.rs

target 目录是编译产出目录
vendor 目录是第三方库目录
```


