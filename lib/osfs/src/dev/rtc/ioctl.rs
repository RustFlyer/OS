use strum::FromRepr;

/// Defined in <linux/rtc.h>
#[allow(non_camel_case_types)]
#[derive(FromRepr, Debug)]
#[repr(u64)]
pub enum RtcIoctlCmd {
    /// Read RTC time into struct rtc_time.
    RTC_RD_TIME = 0x80247009,
    /// Set RTC time from struct rtc_time.
    RTC_SET_TIME = 0x4024700a,
    /// Read RTC alarm time into struct rtc_time.
    RTC_ALM_READ = 0x80247008,
    /// Set RTC alarm time from struct rtc_time.
    RTC_ALM_SET = 0x40247007,
    /// Read RTC interrupt flags.
    RTC_IRQP_READ = 0x8004700b,
    /// Set RTC interrupt frequency.
    RTC_IRQP_SET = 0x4004700c,
    /// Enable periodic interrupts.
    RTC_PIE_ON = 0x7005,
    /// Disable periodic interrupts.
    RTC_PIE_OFF = 0x7006,
    /// Enable alarm interrupts.
    RTC_AIE_ON = 0x7001,
    /// Disable alarm interrupts.
    RTC_AIE_OFF = 0x7002,
    /// Enable update interrupts.
    RTC_UIE_ON = 0x7003,
    /// Disable update interrupts.
    RTC_UIE_OFF = 0x7004,
    /// Read periodic interrupt frequency.
    RTC_EPOCH_READ = 0x8004700d,
    /// Set epoch.
    RTC_EPOCH_SET = 0x4004700e,
    /// Read current RTC interrupt frequency.
    RTC_WKALM_RD = 0x80287010,
    /// Set RTC wake alarm.
    RTC_WKALM_SET = 0x4028700f,
}
