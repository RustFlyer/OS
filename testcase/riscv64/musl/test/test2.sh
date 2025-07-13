#!/bin/bash

# 测试脚本 - 诊断loop设备问题
set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查loop设备状态
check_loop_devices() {
    log_info "=== 检查loop设备状态 ==="
    
    log_info "可用的loop设备："
    ls -la /dev/loop* 2>/dev/null || log_warn "没有找到loop设备"
    
    log_info "当前loop设备状态："
    losetup -l 2>/dev/null || log_warn "losetup -l 失败"
    
    log_info "检查内核模块："
    lsmod | grep loop || log_warn "loop模块未加载"
}

# 详细测试losetup
test_losetup_detailed() {
    log_info "=== 详细测试losetup ==="
    
    local img_file="$TST_TMPDIR/test_dev.img"
    local loop_dev="/dev/loop7"
    
    log_info "创建测试镜像文件..."
    dd if=/dev/zero of="$img_file" bs=1M count=10 2>/dev/null
    
    log_info "测试镜像文件创建成功: $(ls -lh "$img_file")"
    
    # 检查loop设备是否已被使用
    log_info "检查 $loop_dev 当前状态..."
    if losetup "$loop_dev" 2>/dev/null; then
        log_warn "$loop_dev 已被使用，尝试释放..."
        losetup -d "$loop_dev" 2>/dev/null || log_warn "释放失败"
    fi
    
    # 手动测试losetup命令
    log_info "手动执行 losetup 命令..."
    echo "执行: losetup $loop_dev $img_file"
    
    if losetup "$loop_dev" "$img_file"; then
        log_info "losetup 成功！"
        
        # 检查设置结果
        log_info "验证loop设备设置..."
        losetup -l | grep loop7 || log_warn "未找到loop7在列表中"
        
        # 检查设备文件
        log_info "检查设备文件状态..."
        ls -la "$loop_dev"
        file "$loop_dev"
        
        # 清理
        log_info "清理loop设备..."
        losetup -d "$loop_dev"
        log_info "loop设备清理完成"
        
        return 0
    else
        local ret=$?
        log_error "losetup 失败，退出码: $ret"
        
        # 尝试获取更多错误信息
        log_info "尝试获取详细错误信息..."
        strace -e trace=openat,ioctl losetup "$loop_dev" "$img_file" 2>&1 || true
        
        return $ret
    fi
}

# 测试其他loop设备
test_other_loops() {
    log_info "=== 测试其他loop设备 ==="
    
    local img_file="$TST_TMPDIR/test_dev.img"
    
    for i in $(seq 0 7); do
        local loop_dev="/dev/loop$i"
        if [ -e "$loop_dev" ]; then
            log_info "测试 $loop_dev..."
            
            # 检查是否已使用
            if ! losetup "$loop_dev" 2>/dev/null; then
                log_info "$loop_dev 空闲，尝试设置..."
                if losetup "$loop_dev" "$img_file" 2>/dev/null; then
                    log_info "$loop_dev 设置成功！"
                    losetup -d "$loop_dev" 2>/dev/null
                    export TST_DEVICE="$loop_dev"
                    return 0
                else
                    log_warn "$loop_dev 设置失败"
                fi
            else
                log_info "$loop_dev 已被使用"
            fi
        fi
    done
    
    log_error "所有loop设备测试失败"
    return 1
}

# 检查内核配置
check_kernel_config() {
    log_info "=== 检查内核配置 ==="
    
    if [ -f /proc/config.gz ]; then
        log_info "检查内核配置中的loop设备支持..."
        zcat /proc/config.gz | grep -i loop || log_warn "未找到loop相关配置"
    else
        log_warn "/proc/config.gz 不存在"
    fi
    
    # 检查/proc/devices
    log_info "检查 /proc/devices 中的块设备..."
    grep -i loop /proc/devices || log_warn "未找到loop设备驱动"
}

# 主函数
main() {
    log_info "开始loop设备问题诊断"
    
    if [ "$(id -u)" -ne 0 ]; then
        log_error "需要root权限"
        exit 1
    fi
    
    # 创建临时目录
    export TST_TMPDIR=$(mktemp -d "/tmp/loop_debug.XXXXXXXXXX")
    cd "$TST_TMPDIR"
    log_info "工作目录: $TST_TMPDIR"
    
    # 清理函数
    cleanup() {
        log_info "清理资源..."
        cd /
        rm -rf "$TST_TMPDIR" 2>/dev/null || true
    }
    trap cleanup EXIT
    
    # 执行各项检查
    check_loop_devices
    check_kernel_config
    test_losetup_detailed
    test_other_loops
    
    if [ -n "$TST_DEVICE" ]; then
        log_info "找到可用的loop设备: $TST_DEVICE"
    else
        log_error "未找到可用的loop设备"
        exit 1
    fi
}

main "$@"