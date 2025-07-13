#!/bin/sh

# 测试脚本 - 用于定位LTP fsetxattr01失败问题
# 使用方法: ./debug_script.sh

set -e  # 遇到错误立即退出

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 模拟LTP环境变量
export TST_ID="debug_fsetxattr"
export TST_COUNT=1
export TST_FS_TYPE="ext2"
export TST_NEEDS_DEVICE=1
export TST_FORMAT_DEVICE=1
export TST_MOUNT_DEVICE=1
export TST_TMPDIR=""
export TST_DEVICE=""
export TST_MNTPOINT=""

# 简化的tst_res函数
tst_res() {
    local res=$1
    shift
    log_info "[$res] $*"
    
    # 测试: 检查这里是否会出问题
    printf "$TST_ID $TST_COUNT " >&2
    echo "[$res]: $@" >&2
}

# 简化的ROD_SILENT函数
ROD_SILENT() {
    log_info "Executing: $*"
    "$@"
    local ret=$?
    if [ $ret -ne 0 ]; then
        log_error "Command failed with exit code: $ret"
        exit $ret
    fi
    return 0
}

# 检查命令是否存在
tst_require_cmds() {
    local cmd
    for cmd in $*; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            log_error "'$cmd' not found"
            exit 1
        fi
        log_info "Command '$cmd' found"
    done
}

# 创建临时目录
setup_tmpdir() {
    log_info "=== 步骤1: 创建临时目录 ==="
    
    if [ -z "$TMPDIR" ]; then
        export TMPDIR="/tmp"
    fi

    TST_TMPDIR=$(mktemp -d "$TMPDIR/LTP_debug.XXXXXXXXXX")
    TST_TMPDIR=$(echo "$TST_TMPDIR" | sed 's~/\+~/~g')
    
    log_info "Created temp directory: $TST_TMPDIR"
    chmod 777 "$TST_TMPDIR"
    cd "$TST_TMPDIR"
    
    export TST_TMPDIR
    log_info "Changed to directory: $(pwd)"
}

# 创建设备镜像
create_device_image() {
    log_info "=== 步骤2: 创建设备镜像 ==="
    
    local img_file="$TST_TMPDIR/test_dev.img"
    local img_size=$((10 * 1024 * 1024))  # 10MB
    
    log_info "Creating device image: $img_file (10MB)"
    
    # 创建镜像文件
    dd if=/dev/zero of="$img_file" bs=1M count=10 2>/dev/null || {
        log_error "Failed to create device image"
        exit 1
    }
    
    log_info "Device image created successfully"
}

# 设置loop设备
setup_loop_device() {
    log_info "=== 步骤3: 设置loop设备 ==="
    
    local img_file="$TST_TMPDIR/test_dev.img"
    
    # 查找可用的loop设备
    for i in $(seq 0 7); do
        local loop_dev="/dev/loop$i"
        if [ -e "$loop_dev" ]; then
            log_info "Checking loop device: $loop_dev"
            
            # 尝试设置loop设备
            if losetup "$loop_dev" "$img_file" 2>/dev/null; then
                TST_DEVICE="$loop_dev"
                log_info "Successfully set up loop device: $TST_DEVICE"
                break
            fi
        fi
    done
    
    if [ -z "$TST_DEVICE" ]; then
        log_error "Failed to set up loop device"
        exit 1
    fi
    
    export TST_DEVICE
}

# 测试tst_res函数
test_tst_res() {
    log_info "=== 步骤4: 测试tst_res函数 ==="
    
    log_info "Testing tst_res with simple message..."
    tst_res TINFO "Simple test message"
    
    log_info "Testing tst_res with variables..."
    tst_res TINFO "Device: $TST_DEVICE, FS: $TST_FS_TYPE"
    
    log_info "Testing tst_res with longer message..."
    tst_res TINFO "This is a longer message to test if there are any buffer overflow issues in the tst_res function implementation"
    
    log_info "tst_res function tests completed successfully"
}

# 测试mkfs命令检查
test_mkfs_check() {
    log_info "=== 步骤5: 测试mkfs命令检查 ==="
    
    log_info "Checking if mkfs.$TST_FS_TYPE exists..."
    tst_require_cmds "mkfs.$TST_FS_TYPE"
    
    log_info "mkfs command check completed successfully"
}

# 模拟tst_mkfs函数
test_tst_mkfs() {
    log_info "=== 步骤6: 测试文件系统格式化 ==="
    
    local fs_type="$TST_FS_TYPE"
    local opts="$TST_DEVICE"
    
    log_info "About to format filesystem..."
    log_info "fs_type=$fs_type, opts=$opts"
    
    # 这里是关键测试点
    log_info "Calling tst_res TINFO before mkfs..."
    tst_res TINFO "Formatting $fs_type with opts='$opts'"
    log_info "tst_res TINFO call completed"
    
    log_info "About to execute mkfs command..."
    ROD_SILENT mkfs.$fs_type $opts
    log_info "mkfs command completed successfully"
}

# 创建挂载点并挂载
test_mount() {
    log_info "=== 步骤7: 测试挂载 ==="
    
    TST_MNTPOINT="$TST_TMPDIR/mntpoint"
    mkdir -p "$TST_MNTPOINT"
    
    log_info "Mounting $TST_DEVICE to $TST_MNTPOINT"
    mount -t "$TST_FS_TYPE" "$TST_DEVICE" "$TST_MNTPOINT"
    
    log_info "Mount successful"
    log_info "Mount info:"
    mount | grep "$TST_DEVICE"
}

# 清理函数
cleanup() {
    log_info "=== 清理资源 ==="
    
    if [ -n "$TST_MNTPOINT" ] && mountpoint -q "$TST_MNTPOINT" 2>/dev/null; then
        log_info "Unmounting $TST_MNTPOINT"
        umount "$TST_MNTPOINT" 2>/dev/null || true
    fi
    
    if [ -n "$TST_DEVICE" ] && losetup "$TST_DEVICE" >/dev/null 2>&1; then
        log_info "Detaching loop device $TST_DEVICE"
        losetup -d "$TST_DEVICE" 2>/dev/null || true
    fi
    
    if [ -n "$TST_TMPDIR" ] && [ -d "$TST_TMPDIR" ]; then
        log_info "Removing temp directory $TST_TMPDIR"
        rm -rf "$TST_TMPDIR" 2>/dev/null || true
    fi
    
    log_info "Cleanup completed"
}

# 主函数
main() {
    log_info "开始LTP fsetxattr01问题调试脚本"
    log_info "当前用户: $(whoami)"
    log_info "当前目录: $(pwd)"
    
    # 设置清理陷阱
    trap cleanup EXIT
    
    # 检查是否有root权限
    if [ "$(id -u)" -ne 0 ]; then
        log_error "This script requires root privileges"
        exit 1
    fi
    
    # 逐步执行测试
    setup_tmpdir
    create_device_image
    setup_loop_device
    test_tst_res
    test_mkfs_check
    test_tst_mkfs
    test_mount
    
    log_info "所有测试步骤完成！"
    log_info "如果到达这里，说明基本的设备操作流程是正常的"
    
    # 显示最终状态
    log_info "=== 最终状态 ==="
    log_info "临时目录: $TST_TMPDIR"
    log_info "设备: $TST_DEVICE"
    log_info "挂载点: $TST_MNTPOINT"
    log_info "文件系统类型: $TST_FS_TYPE"
    
    # 保持一段时间以便检查
    log_info "等待10秒后自动清理..."
    sleep 10
}

# 执行主函数
main "$@"