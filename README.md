![哈工大深圳](./docs/assets/hitsz-logo.jpg)

# Nighthawk OS

## 项目描述

Nighthawk OS 是使用 Rust 编写，支持 RISC-V 和 LoongArch 指令集架构，采用异步无栈协程架构的操作系统。

## 完成情况

### 初赛

<!-- 这里写完成测例情况，并在assets文件夹下放一张得分榜截图leaderboard-pre.png -->
<!--  ![决赛第一阶段排行榜](./docs/assets/leaderboard-pre.png) -->

### 功能介绍
<!--  这里复制了Phoenix的，请检查修改自己了解的部分，为避免重复可以重构或添加模块 -->
- 无栈协程：基于全局队列实现的调度器，完善的辅助 Future 支持，支持内核态抢占式调度。
- 进程管理：统一的进程线程抽象，可以细粒度划分进程共享的资源，支持多核运行。
- 内存管理：实现基本的内存管理功能。使用懒分配和 Copy-on-Write 优化策略。
- 文件系统：基于 Linux 设计的虚拟文件系统。实现页缓存加速文件读写，实现 Dentry 缓存加速路径查找，统一了页缓存与块缓存。使用开源 `rust-fatfs`库提供对 FAT32 文件系统的支持，使用`lwext4-rust`库提供对 Ext4 文件系统的支持。
- 进程通信：实现了符合POSIX标准的信号系统，支持用户自定义信号处理例程；实现了共享内存通信，适配内核其他异步功能。
- 设备驱动：实现设备树解析，实现 PLIC，支持异步外设中断，实现异步串口驱动。
- 网络模块：支持 Udp 和 Tcp 套接字，Ipv4 与 Ipv6 协议，实现异步轮询唤醒机制。

<img src="./docs/assets/Nighthawk-design.png" alt="Nighthawk内核架构" width="450"/>

### 项目文档

<!--  [Nighthawk-初赛文档](./Nighthawk-初赛文档.pdf) -->

## 运行方式

于主目录下

键入 `make build2docker` 即可构建docker环境，

键入 `make docker` 即可进入docker环境，

键入 `make user` 即可编译用户程序，

键入 `make run` 即可运行

键入 `make run-debug` 即可运行debug模式，此模式下会打印when_debug!宏的输出

键入 `make run-docker` 即可在docker中运行

键入 `make run-docker-debug` 即可在docker中运行debug模式，此模式下会打印when_debug!宏的输出

目前项目代码结构如下图，项目代码主要位于 `kernel` 和 `lib` 目录下。
<!-- 这里后来新加的文件夹我还没有细看，可以考虑进一步细化 -->
```
kernel
├── build.rs           ---- link.ld地址替换
├── Cargo.toml         ---- 项目cargo设置
├── link.ld            ---- 链接脚本
├── Makefile           ---- 编译配置
└── src
    ├── boot.rs        ---- 启动多cpu 
    ├── lang_item.rs   ---- 崩溃处理
    ├── linkapp-la.asm ---- 链接应用(LoongArch)
    ├── linkapp-rv.asm ---- 链接应用(RISC-V)
    ├── loader.rs      ---- 加载应用
    ├── logging.rs     ---- 日志打印函数
    ├── main.rs        ---- 主函数
    ├── entry          ---- 多架构入口函数
    │   ├── loongarch64.rs
    │   └── riscv64.rs
    ├── net            ---- 网络系统调用
    ├── osdriver       ---- 操作系统驱动
    ├── processor      ---- 处理器
    │   ├── guard.rs   ---- 多核保护锁
    │   ├── hart.rs    ---- 多核主模块
    │   └── mod.rs     ---- 对外接口
    ├── syscall        ---- 系统调用
    ├── task           ---- 任务
    │   ├── future.rs  ---- 异步调度
    │   ├── manager.rs ---- 任务管理
    │   ├── mod.rs     ---- 对外接口
    │   ├── task.rs    ---- 任务主模块
    │   └── tid.rs     ---- 任务id
    ├── trap           ---- 中断处理
    └── vm             ---- 虚拟内存

lib
├── arch            ---- 特定架构汇编封装
├── config          ---- 配置数据文件
├── driver          ---- 驱动
├── executor        ---- 异步任务执行器
├── ext4            ---- EXT4文件系统支持
├── fat32           ---- FAT32文件系统支持
├── id_allocator    ---- id分配器
├── logger          ---- 日志输出
├── mm              ---- 内存管理
├── mutex           ---- 互斥锁
├── net             ---- 网络模块
├── osfs            ---- 操作系统文件系统接口
├── osfuture        ---- 异步设计
├── polyhal-macro   ---- 架构抽象代码宏
├── pps             ---- cpu特权寄存器存储
├── shm             ---- 共享内存
├── simdebug        ---- 简单调试
├── systype         ---- 系统错误类型
├── timer           ---- 定时器
└── vfs             ---- 虚拟文件系统

target 目录是编译产出目录，可使用 `make build` 生成
vendor 目录是第三方库目录，可使用 `cargo vendor` 生成，用于本地缓存
```

## 项目人员

哈尔滨工业大学（深圳）:

- 关雄正 (<待填>)：进程管理、内存管理、文件系统设计。
- 王峻阳 (<adong660@foxmail.com>)：内存管理、
- 冼志炜 (<18023803967@163.com>)：异常机制、进程间通信
- 指导老师：夏文，仇洁婷

<!-- 参考部分，我们基本只参考了 byteOS 的 HAL 和 Phoenix，写出来不太好看，不知道怎么处理 -->
