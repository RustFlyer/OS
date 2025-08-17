import re

def process_ltp_statistics(input_filename='ltp-statistic.txt', 
                           filtered_filename='ltp-filtered-sorted.txt', 
                           names_filename='ltp-test-names.txt'):
    """
    读取LTP统计文件，根据要求筛选和排序，并生成两个新的结果文件。

    Args:
        input_filename (str): 原始的LTP统计文件名。
        filtered_filename (str): 筛选并排序后的测例列表文件名。
        names_filename (str): 只包含格式化测例名的文件名。
    """
    
    # 定义需要保留的 type 类型
    types_to_keep = {'all pass', 'skip pass', 'part fail', 'skip'}
    
    filtered_test_cases = []

    try:
        with open(input_filename, 'r', encoding='utf-8') as f:
            lines = f.readlines()
    except FileNotFoundError:
        print(f"错误: 输入文件 '{input_filename}' 未找到。")
        return

    # --- 1. 解析并筛选数据 ---
    for line in lines:
        clean_line = line.strip()
        # 跳过注释、表头或空行
        if not clean_line or clean_line.startswith('//') or clean_line.lower().startswith('name'):
            continue

        # 使用正则表达式分割，以处理不规则的空白符
        parts = re.split(r'\s+', clean_line)
        if len(parts) < 4:
            continue

        try:
            name = parts[0]
            fails_broken = int(parts[-1])
            passes = int(parts[-2])
            test_type = " ".join(parts[1:-2])

            # 检查 test_type 是否在我们想要保留的集合中
            if test_type in types_to_keep:
                filtered_test_cases.append({
                    'name': name,
                    'type': test_type,
                    'pass': passes,
                    'fail_broken': fails_broken,
                    'original_line': clean_line # 保留原始行以便写入
                })

        except (ValueError, IndexError):
            # 跳过格式不正确的行
            print(f"警告: 无法解析此行: {clean_line}")
            continue

    # --- 2. 按 pass 数量降序排序 ---
    sorted_cases = sorted(filtered_test_cases, key=lambda x: x['pass'], reverse=True)

    # --- 3. 写入第一个文件 (筛选并排序后的列表) ---
    with open(filtered_filename, 'w', encoding='utf-8') as f:
        # 写入表头
        f.write("name\t\t\ttype\t\tpass\tfail+broken\n")
        
        for case in sorted_cases:
            # 为了保持大致的对齐，我们用制表符分隔
            f.write(f"{case['name']}\t\t{case['type']}\t{case['pass']}\t{case['fail_broken']}\n")
    
    print(f"成功生成已筛选和排序的文件: '{filtered_filename}'")

    # --- 4. 写入第二个文件 (格式化的测例名) ---
    with open(names_filename, 'w', encoding='utf-8') as f:
        for case in sorted_cases:
            f.write(f'"{case["name"]}",\n')
            
    print(f"成功生成测例名文件: '{names_filename}'")


if __name__ == '__main__':
    # 确保 'ltp-statistic.txt' 文件在此脚本的同一目录下
    process_ltp_statistics()