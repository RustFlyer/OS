import re
import math

def create_ranking_report(input_filename='ltp-statistic.txt', 
                          output_filename='ltp-ranking-final.txt',
                          failure_count_weight=0.5):
    """
    读取LTP测试统计文件，根据一个复合权重公式生成排行榜，以帮助定位需要优先修正的测例。

    权重策略:
    - 综合考虑“通过率”、“总单元数”和“总失败数（收益）”。
    - 引入可配置的`failure_count_weight`来调整对“收益”的重视程度。
    - score = [(pass / fail_broken) / total_units] + [failure_count_weight * log(1 + fail_broken)]

    Args:
        input_filename (str): 输入的LTP统计文件名。
        output_filename (str): 输出的排行榜文件名。
        failure_count_weight (float): 用于调整总失败单元数在分数中所占权重的系数。
                                      调高此值会优先展示失败数多的测例。
    """
    test_cases = []

    try:
        with open(input_filename, 'r', encoding='utf-8') as f:
            lines = f.readlines()
    except FileNotFoundError:
        print(f"错误: 未找到输入文件 '{input_filename}'。")
        return

    # --- 1. 解析文件数据 ---
    for line in lines:
        clean_line = line.strip()
        if not clean_line or clean_line.startswith('//') or clean_line.lower().startswith('name'):
            continue

        parts = re.split(r'\s+', clean_line)
        if len(parts) < 4:
            continue

        try:
            name = parts[0]
            fails_broken = int(parts[-1])
            passes = int(parts[-2])
            test_type = " ".join(parts[1:-2])
            total_units = passes + fails_broken

            # --- 2. 计算新的复合权重分数 ---
            score = 0.0
            # 必须有失败的单元才有修正的意义
            if fails_broken > 0 and total_units > 0:
                # Part 1: (通过率 / 总单元数) -> 惩罚低通过率和高复杂度的测例
                base_score = (passes / fails_broken) / total_units
                
                # Part 2: (失败数权重 * log(总失败数)) -> 奖励失败数多的高收益测例
                # 使用 log(1+x) 来避免 log(0) 并平滑收益曲线
                reward_score = failure_count_weight * math.log(1 + fails_broken)
                
                score = base_score + reward_score

            test_cases.append({
                'name': name,
                'type': test_type,
                'pass': passes,
                'fail_broken': fails_broken,
                'total': total_units,
                'score': score
            })

        except (ValueError, IndexError):
            print(f"警告: 无法解析该行: {clean_line}")
            continue

    # --- 3. 按权重分数（score）降序排序 ---
    ranked_cases = sorted(test_cases, key=lambda x: x['score'], reverse=True)

    # --- 4. 将排序结果写入新文件 ---
    with open(output_filename, 'w', encoding='utf-8') as f:
        f.write("LTP 测例修正优先级排行榜 (按修正潜力排行)\n")
        f.write("==================================================\n\n")
        f.write(f"排序依据: score = (pass/fail)/total + {failure_count_weight}*log(1+fail), 分数越高优先级越高。权重可在rank.py调整。\n\n")
        
        header = f"{'Rank':<6}{'Name':<30}{'Type':<25}{'Pass':<6}{'Fail/Broken':<12}{'Total':<8}{'Score':<10}\n"
        f.write(header)
        f.write('-' * (len(header) + 5) + '\n')

        for i, case in enumerate(ranked_cases, 1):
            f.write(
                f"{i:<6}"
                f"{case['name']:<30}"
                f"{case['type']:<25}"
                f"{case['pass']:<6}"
                f"{case['fail_broken']:<12}"
                f"{case['total']:<8}"
                f"{case['score']:.4f}\n"
            )
            
    print(f"排行榜已成功生成: '{output_filename}'")

if __name__ == '__main__':
    # ===============================================================
    # 在这里修改 `failure_count_weight` 的值来调整您对“失败单元数”的重视程度
    # 默认值是 0.5
    # ===============================================================
    weight_for_failures = 0.3
    
    create_ranking_report(input_filename='ltp-statistic.txt', 
                          output_filename='ltp-ranking-final.txt',
                          failure_count_weight=weight_for_failures)
