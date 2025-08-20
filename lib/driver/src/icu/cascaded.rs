use super::{ICU, ehic::LoongArchEIOINTC, icu_lavirt::TriggerType, pch::LoongArchPCHPIC};

pub struct CascadedICU {
    pub eiointc: LoongArchEIOINTC,
    pub pch_pic: LoongArchPCHPIC,
    /// PCH-PIC 在 EIOINTC 中使用的中断线范围
    pch_irq_base: usize,
    pch_irq_count: usize,
}

impl CascadedICU {
    pub fn new(
        eiointc: LoongArchEIOINTC,
        pch_pic: LoongArchPCHPIC,
        pch_irq_base: usize,
        pch_irq_count: usize,
    ) -> Self {
        Self {
            eiointc,
            pch_pic,
            pch_irq_base,
            pch_irq_count,
        }
    }

    /// 检查一个 EIOINTC 中断号是否属于 PCH-PIC
    fn is_pch_irq(&self, eiointc_irq: usize) -> bool {
        eiointc_irq >= self.pch_irq_base && eiointc_irq < self.pch_irq_base + self.pch_irq_count
    }
}

impl CascadedICU {
    /// 获取PCH-PIC IRQ在EIOINTC中的映射
    fn get_eiointc_vector(&self, pch_irq: usize) -> usize {
        // 使用base_vec + irq作为向量号
        self.pch_pic.base_vec as usize + pch_irq
    }

    /// 检查向量是否来自PCH-PIC
    fn is_pch_vector(&self, vector: usize) -> bool {
        vector >= self.pch_pic.base_vec as usize && vector < (self.pch_pic.base_vec + 64) as usize
    }
}

impl ICU for CascadedICU {
    fn enable_irq(&self, irq: usize, ctx_id: usize) {
        // 1. 在PCH-PIC中配置并使能
        self.pch_pic.enable_irq(irq, ctx_id);

        // 2. 在EIOINTC中使能对应的向量
        let vector = self.get_eiointc_vector(irq);
        self.eiointc.enable_irq(vector, ctx_id);

        log::debug!(
            "CascadedICU: enabled IRQ {} (vector {}) for context {}",
            irq,
            vector,
            ctx_id
        );
    }

    fn disable_irq(&self, irq: usize) {
        todo!()
    }

    fn claim_irq(&self, ctx_id: usize) -> Option<usize> {
        // 从EIOINTC获取中断向量
        if let Some(vector) = self.eiointc.claim_irq(ctx_id) {
            // 检查是否是PCH-PIC的向量
            if self.is_pch_vector(vector) {
                // 转换为PCH-PIC的IRQ号
                let pch_irq = vector - self.pch_pic.base_vec as usize;
                log::trace!(
                    "CascadedICU: claimed PCH IRQ {} from vector {}",
                    pch_irq,
                    vector
                );
                return Some(pch_irq);
            }

            // 非PCH-PIC的中断，直接返回向量号
            log::trace!("CascadedICU: claimed direct vector {}", vector);
            return Some(vector);
        }

        None
    }

    fn complete_irq(&self, irq: usize, cpu_id: usize) {
        // 判断是否需要清除PCH-PIC
        if irq < 64 {
            // 这是PCH-PIC的中断
            self.pch_pic.complete_irq(irq, cpu_id);

            // 清除EIOINTC中对应的向量
            let vector = self.get_eiointc_vector(irq);
            self.eiointc.complete_irq(vector, cpu_id);
        } else {
            // 直接的EIOINTC中断
            self.eiointc.complete_irq(irq, cpu_id);
        }
    }

    fn set_trigger_type(&self, irq: usize, trigger: TriggerType) {
        // 只在PCH-PIC层设置，EIOINTC使用固定的接收方式
        if irq < 64 {
            self.pch_pic.set_trigger_type(irq, trigger);
        }
    }
}
