目前架构如图，已写代码文件位于 `kernel` 和 `lib` 目录下

于主目录下

键入 `make build2docker` 即可构建docker环境，

键入 `make docker` 即可进入docker环境，

键入 `make user` 即可编译用户程序，

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
    ├── boot.rs        ---- 启动多cpu 
    ├── console.rs     ---- 打印函数
    ├── entry.rs       ---- 入口函数
    ├── lang_item.rs   ---- 崩溃处理
    ├── link_app.asm   ---- 链接应用
    ├── loader.rs      ---- 加载应用
    ├── logging.rs     ---- 日志打印函数
    ├── main.rs        ---- 主函数
    ├── processor      ---- 处理器
    │   ├── guard.rs   ---- 多核保护锁
    │   ├── hart.rs    ---- 多核主模块
    │   └── mod.rs     ---- 对外接口
    ├── sbi.rs         ---- 硬件调用接口
    └── task           ---- 任务
        ├── future.rs  ---- 异步调度
        ├── manager.rs ---- 任务管理
        ├── mod.rs     ---- 对外接口
        ├── task.rs    ---- 任务主模块
        └── tid.rs     ---- 任务id

lib
├── arch            ---- 特定架构汇编封装
├── config          ---- 配置数据文件
├── driver          ---- 驱动
├── executor        ---- 异步任务执行器
├── id_allocator    ---- id分配器
├── logger          ---- 日志输出
├── mm              ---- 内存管理
├── mutex           ---- 互斥锁
├── pps             ---- cpu特权寄存器存储
├── simdebug        ---- 简单调试
├── systype         ---- 系统错误类型
├── time            ---- 时间管理
└── timer           ---- 定时器

target 目录是编译产出目录，可使用 `make build` 生成
vendor 目录是第三方库目录，可使用 `cargo vendor` 生成，用于本地缓存
```


